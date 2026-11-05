package org.netsketch.survey;

import android.content.Context;
import android.content.pm.PackageManager;
import android.net.ConnectivityManager;
import android.net.LinkAddress;
import android.net.LinkProperties;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.net.RouteInfo;
import android.net.nsd.NsdManager;
import android.net.nsd.NsdServiceInfo;
import android.net.wifi.ScanResult;
import android.net.wifi.WifiManager;
import android.net.wifi.rtt.RangingRequest;
import android.net.wifi.rtt.RangingResult;
import android.net.wifi.rtt.RangingResultCallback;
import android.net.wifi.rtt.WifiRttManager;
import android.os.Build;
import android.os.SystemClock;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.ConnectException;
import java.net.DatagramSocket;
import java.net.HttpURLConnection;
import java.net.Inet4Address;
import java.net.Inet6Address;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.net.URL;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.function.BiConsumer;

/**
 * The Android side of looking around: reading a scan result (with its information elements), Android's
 * scan budget, Wi-Fi RTT distances, the router's address and a knock on it, what the internet side looks
 * like (UPnP, public address, IPv6, DNS) and which devices are on the network (SSDP, mDNS, a port knock).
 * The parsing lives in plain Java (Air, Lan) so it can be checked on a computer.
 */
final class Scope {
    private Scope() {}

    /** Android 9+ lets an app in the foreground start 4 Wi-Fi scans every 2 minutes (a foreground service counts). */
    static final int SCAN_BUDGET = 4;
    static final long SCAN_WINDOW_MS = 120_000;
    /** Scan starts of this app (Android's count, so they outlive the screen). */
    private static final List<Long> scanTimes = new ArrayList<>();

    /** How long until Android allows this app another scan (0 = now). */
    static synchronized long scanWaitMs(WifiManager wifi) {
        if (Build.VERSION.SDK_INT >= 30 && !wifi.isScanThrottleEnabled()) return 0;
        long now = SystemClock.elapsedRealtime();
        scanTimes.removeIf(t -> now - t > SCAN_WINDOW_MS);
        if (scanTimes.size() < SCAN_BUDGET) return 0;
        return SCAN_WINDOW_MS - (now - scanTimes.get(0)) + 500;
    }

    static synchronized int scansLeft(WifiManager wifi) {
        if (Build.VERSION.SDK_INT >= 30 && !wifi.isScanThrottleEnabled()) return SCAN_BUDGET;
        scanWaitMs(wifi);
        return SCAN_BUDGET - scanTimes.size();
    }

    @SuppressWarnings("deprecation")
    static synchronized boolean startScan(WifiManager wifi) {
        if (!wifi.startScan()) return false;
        scanTimes.add(SystemClock.elapsedRealtime());
        return true;
    }

    // ---------- reading a scan ----------

    /** What a scan says about one network (its strength is this one reading). */
    static Survey.Seen seenOf(ScanResult r) {
        Survey.Seen s = new Survey.Seen();
        s.ssid = r.SSID == null ? "" : r.SSID;
        s.bssid = r.BSSID == null ? "" : r.BSSID;
        s.freq = r.frequency;
        s.channel = Survey.channelOf(r.frequency);
        s.width = Survey.widthOf(r.channelWidth);
        s.caps = r.capabilities == null ? "" : r.capabilities;
        s.security = Survey.securityOf(r.capabilities);
        s.rssi = r.level;
        s.ftm = r.is80211mcResponder();
        s.tsUs = r.timestamp;
        long nowMs = System.currentTimeMillis(), bootUs = SystemClock.elapsedRealtime() * 1000;
        s.at = r.timestamp > 0 ? Math.min(nowMs, nowMs - (bootUs - r.timestamp) / 1000) : nowMs;
        if (Build.VERSION.SDK_INT >= 30) {
            s.standard = Survey.standardOf(r.getWifiStandard());
            for (ScanResult.InformationElement ie : r.getInformationElements()) {
                ByteBuffer b = ie.getBytes();
                byte[] a = new byte[b.remaining()];
                b.get(a);
                // Android 11 hides the extension id, so HE (Wi-Fi 6) streams need Android 12.
                Air.read(s, ie.getId(), Build.VERSION.SDK_INT >= 31 ? ie.getIdExt() : -1, a);
            }
        }
        return s;
    }

    /** The phone's latest scan results (empty without permission). */
    static List<ScanResult> results(WifiManager wifi) {
        try {
            List<ScanResult> l = wifi.getScanResults();
            return l == null ? Collections.emptyList() : l;
        } catch (SecurityException e) {
            return Collections.emptyList();
        }
    }

    /**
     * Hands the latest scan to the tracker (every access point heard over time); channel changes go into
     * the watch log while watching. With range, also asks Wi-Fi RTT for distances.
     */
    static void feed(Context c, WifiManager wifi, boolean range) {
        List<ScanResult> rs = results(wifi);
        List<Survey.Seen> seen = new ArrayList<>();
        for (ScanResult r : rs) seen.add(seenOf(r));
        List<Survey.Event> changes = Air.TRACKER.update(seen);
        Watch w = Watch.get();
        if (w.active) for (Survey.Event e : changes) w.event(e);
        if (range) range(c, rs, Air.TRACKER::distance);
    }

    // ---------- Wi-Fi RTT ----------

    static boolean rttSupported(Context c) {
        return Build.VERSION.SDK_INT >= 28 && c.getPackageManager().hasSystemFeature(PackageManager.FEATURE_WIFI_RTT);
    }

    /** Distances (m) to the access points that answer Wi-Fi RTT (802.11mc); quietly nothing if the phone can't. */
    static void range(Context c, List<ScanResult> rs, BiConsumer<String, Double> each) {
        if (!rttSupported(c)) return;
        WifiRttManager m = c.getSystemService(WifiRttManager.class);
        if (m == null || !m.isAvailable()) return;
        RangingRequest.Builder b = new RangingRequest.Builder();
        int n = 0;
        for (ScanResult r : rs) {
            if (!r.is80211mcResponder() || n >= RangingRequest.getMaxPeers()) continue;
            b.addAccessPoint(r);
            n++;
        }
        if (n == 0) return;
        try {
            m.startRanging(b.build(), c.getMainExecutor(), new RangingResultCallback() {
                @Override
                public void onRangingFailure(int code) {
                    // no distances this time
                }

                @Override
                public void onRangingResults(List<RangingResult> results) {
                    for (RangingResult x : results) {
                        if (x.getStatus() == RangingResult.STATUS_SUCCESS && x.getMacAddress() != null) each.accept(x.getMacAddress().toString(), x.getDistanceMm() / 1000.0);
                    }
                }
            });
        } catch (SecurityException | IllegalArgumentException | IllegalStateException e) {
            // not allowed or busy: skip
        }
    }

    // ---------- the router ----------

    /** The phone's Wi-Fi network (not mobile data), or null. */
    @SuppressWarnings("deprecation")
    static Network wifiNetwork(Context c) {
        ConnectivityManager cm = c.getSystemService(ConnectivityManager.class);
        if (cm == null) return null;
        for (Network n : cm.getAllNetworks()) {
            NetworkCapabilities nc = cm.getNetworkCapabilities(n);
            if (nc != null && nc.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) return n;
        }
        return null;
    }

    /** The Wi-Fi's router (its default gateway), IPv4 if there is one. */
    static InetAddress gateway(Context c, Network net) {
        LinkProperties lp = net == null ? null : c.getSystemService(ConnectivityManager.class).getLinkProperties(net);
        if (lp == null) return null;
        InetAddress other = null;
        for (RouteInfo r : lp.getRoutes()) {
            InetAddress g = r.getGateway();
            if (!r.isDefaultRoute() || g == null || g.isAnyLocalAddress()) continue;
            if (g instanceof Inet4Address) return g;
            if (other == null) other = g;
        }
        return other;
    }

    /**
     * One round trip to the router over Wi-Fi, in ms (null = no answer in time). It opens a TCP
     * connection to the router's DNS port: an accepted or refused connection both answer, and it needs no root.
     * shortcut: TCP through the router's own stack is a little slower than an ICMP ping; fine for spotting a bad spot.
     */
    static Double knock(Network net, InetAddress gw, int timeoutMs) {
        long t0 = System.nanoTime();
        try (Socket s = net.getSocketFactory().createSocket()) {
            s.connect(new InetSocketAddress(gw, 53), timeoutMs);
            return (System.nanoTime() - t0) / 1e6;
        } catch (ConnectException e) {
            String m = e.getMessage();
            return m != null && m.contains("refused") ? (System.nanoTime() - t0) / 1e6 : null;
        } catch (Exception e) {
            return null;
        }
    }

    /** A quick round of knocks (for a measuring point); null if not on Wi-Fi or nothing answered. */
    static Survey.Ping quickPing(Context c, int tries) {
        Network net = wifiNetwork(c);
        InetAddress gw = gateway(c, net);
        if (gw == null) return null;
        List<Double> ms = new ArrayList<>();
        for (int i = 0; i < tries; i++) {
            Double t = knock(net, gw, 500);
            if (t != null) ms.add(t);
            SystemClock.sleep(120);
        }
        return Survey.Ping.of(ms, tries);
    }

    // ---------- the internet side ----------

    static String get(Network net, String url, int timeoutMs) {
        try {
            HttpURLConnection c = (HttpURLConnection) net.openConnection(new URL(url));
            c.setConnectTimeout(timeoutMs);
            c.setReadTimeout(timeoutMs);
            c.setRequestProperty("User-Agent", "NetSketch-Survey/0.3");
            try (InputStream in = c.getInputStream()) {
                return read(in, 256 * 1024);
            } finally {
                c.disconnect();
            }
        } catch (Exception e) {
            return null;
        }
    }

    private static String read(InputStream in, int max) throws java.io.IOException {
        ByteArrayOutputStream all = new ByteArrayOutputStream();
        byte[] buf = new byte[8192];
        int n;
        while ((n = in.read(buf)) > 0 && all.size() < max) all.write(buf, 0, n);
        return all.toString("UTF-8");
    }

    /** Asks for UPnP devices of one kind on the Wi-Fi (st = "ssdp:all" for every kind). */
    static List<Lan.Answer> ssdp(Network net, String st, int ms) {
        try (DatagramSocket s = new DatagramSocket()) {
            net.bindSocket(s);
            return Lan.search(s, st, ms);
        } catch (Exception e) {
            return Collections.emptyList();
        }
    }

    /**
     * The router's external address over UPnP (null if it doesn't answer), the public address the internet
     * sees, whether IPv6 reaches the internet, how long a name lookup takes, and which DNS servers are used.
     * Takes a few seconds; call it off the main thread.
     */
    static Assistant.Facts facts(Context c) {
        Assistant.Facts f = new Assistant.Facts();
        Network net = wifiNetwork(c);
        if (net == null) return f;
        f.checked = true;
        InetAddress gw = gateway(c, net);
        WifiManager.MulticastLock lock = c.getApplicationContext().getSystemService(WifiManager.class).createMulticastLock("netsketch-upnp");
        lock.acquire();
        try {
            List<Lan.Answer> igd = ssdp(net, Lan.IGD, 2500);
            for (Lan.Answer a : igd) {
                if (gw != null && !a.from.equals(gw.getHostAddress()) && igd.size() > 1) continue;
                String desc = get(net, a.location, 2500);
                if (desc == null) continue;
                String maker = Lan.tag(desc, "manufacturer"), model = Lan.tag(desc, "modelName");
                f.router = (model.toLowerCase().startsWith(maker.toLowerCase()) || maker.isEmpty() ? model : maker + " " + model).trim();
                String[] wan = Lan.wanControl(desc, a.location);
                if (wan != null) f.upnpIp = soapExternalIp(net, wan[0], wan[1]);
                break;
            }
        } finally {
            lock.release();
        }
        f.publicIp = Lan.traceIp(get(net, "https://1.1.1.1/cdn-cgi/trace", 5000));
        String any = Lan.traceIp(get(net, "https://speed.cloudflare.com/cdn-cgi/trace", 5000));
        if (f.publicIp == null && Lan.isV4(any)) f.publicIp = any;
        f.ipv6 = (any != null && any.contains(":")) || get(net, "https://[2606:4700:4700::1111]/cdn-cgi/trace", 4000) != null;
        // A name nobody asked for yet, so no cache answers it: the time the whole lookup takes.
        List<Double> ms = new ArrayList<>();
        for (int i = 0; i < 3; i++) {
            long t0 = System.nanoTime();
            try {
                net.getAllByName("ns" + Long.toHexString(System.nanoTime()) + i + ".cloudflare.com");
            } catch (Exception ignored) {
                // "no such name" is the answer we time
            }
            ms.add((System.nanoTime() - t0) / 1e6);
        }
        Collections.sort(ms);
        f.dnsMs = ms.get(1);
        LinkProperties lp = c.getSystemService(ConnectivityManager.class).getLinkProperties(net);
        if (lp != null) {
            for (InetAddress d : lp.getDnsServers()) f.dns.add(d.getHostAddress());
            if (lp.isPrivateDnsActive()) f.privateDns = lp.getPrivateDnsServerName() == null ? "Private DNS" : lp.getPrivateDnsServerName();
        }
        return f;
    }

    private static String soapExternalIp(Network net, String url, String type) {
        try {
            HttpURLConnection c = (HttpURLConnection) net.openConnection(new URL(url));
            c.setConnectTimeout(2500);
            c.setReadTimeout(2500);
            c.setRequestMethod("POST");
            c.setDoOutput(true);
            c.setRequestProperty("Content-Type", "text/xml; charset=\"utf-8\"");
            c.setRequestProperty("SOAPAction", "\"" + type + "#GetExternalIPAddress\"");
            byte[] body = Lan.soapExternalIp(type).getBytes(StandardCharsets.UTF_8);
            c.setFixedLengthStreamingMode(body.length);
            try (OutputStream out = c.getOutputStream()) {
                out.write(body);
            }
            try (InputStream in = c.getInputStream()) {
                String ip = Lan.tag(read(in, 64 * 1024), "NewExternalIPAddress");
                return ip.isEmpty() ? null : ip;
            } finally {
                c.disconnect();
            }
        } catch (Exception e) {
            return null;
        }
    }

    // ---------- devices on the network ----------

    /**
     * Who is on the Wi-Fi's network: UPnP devices (with their own name, maker and model), mDNS services
     * (printers, TVs, speakers, smart-home), and a quick knock on common ports of every address in the /24.
     * About 10 s; call it off the main thread. progress gets short status texts.
     */
    static List<Survey.LanDevice> devices(Context c, java.util.function.Consumer<String> progress) {
        Network net = wifiNetwork(c);
        List<Survey.LanDevice> none = new ArrayList<>();
        if (net == null) return none;
        Map<String, Survey.LanDevice> by = new LinkedHashMap<>();
        InetAddress gw = gateway(c, net);
        LinkProperties lp = c.getSystemService(ConnectivityManager.class).getLinkProperties(net);
        Inet4Address me = null;
        if (lp != null) for (LinkAddress la : lp.getLinkAddresses()) if (la.getAddress() instanceof Inet4Address) me = (Inet4Address) la.getAddress();
        WifiManager.MulticastLock lock = c.getApplicationContext().getSystemService(WifiManager.class).createMulticastLock("netsketch-lan");
        lock.acquire();
        NsdManager nsd = c.getSystemService(NsdManager.class);
        List<NsdManager.DiscoveryListener> listeners = new ArrayList<>();
        ConcurrentLinkedQueue<NsdServiceInfo> found = new ConcurrentLinkedQueue<>();
        try {
            // mDNS runs in the background while the rest is asked.
            for (String type : Lan.SERVICES) {
                NsdManager.DiscoveryListener l = new NsdManager.DiscoveryListener() {
                    @Override public void onStartDiscoveryFailed(String t, int e) {}
                    @Override public void onStopDiscoveryFailed(String t, int e) {}
                    @Override public void onDiscoveryStarted(String t) {}
                    @Override public void onDiscoveryStopped(String t) {}
                    @Override public void onServiceFound(NsdServiceInfo s) {
                        found.add(s);
                    }
                    @Override public void onServiceLost(NsdServiceInfo s) {}
                };
                try {
                    nsd.discoverServices(type, NsdManager.PROTOCOL_DNS_SD, l);
                    listeners.add(l);
                } catch (RuntimeException ignored) {
                    // that type can't be asked for now
                }
            }
            progress.accept(T.t("Asking UPnP devices…"));
            for (Lan.Answer a : ssdp(net, "ssdp:all", 3000)) {
                String desc = get(net, a.location, 1500);
                Survey.LanDevice d = by.computeIfAbsent(a.from, Scope::device);
                if (desc == null) continue;
                if (d.name.isEmpty()) d.name = Lan.tag(desc, "friendlyName");
                if (d.vendor.isEmpty()) d.vendor = Lan.tag(desc, "manufacturer");
                if (d.model.isEmpty()) d.model = Lan.tag(desc, "modelName");
                if (d.kind.isEmpty()) d.kind = Lan.kindOfUpnp(Lan.tag(desc, "deviceType"));
            }
            if (me != null) {
                progress.accept(T.t("Knocking on every address…"));
                sweep(net, me, by);
            }
            progress.accept(T.t("Listening for printers, TVs and speakers…"));
            SystemClock.sleep(1500);
        } finally {
            for (NsdManager.DiscoveryListener l : listeners) {
                try {
                    nsd.stopServiceDiscovery(l);
                } catch (RuntimeException ignored) {
                    // already stopped
                }
            }
            lock.release();
        }
        resolveAll(nsd, found, by);
        if (gw != null) {
            Survey.LanDevice r = by.computeIfAbsent(gw.getHostAddress(), Scope::device);
            r.kind = "Router";
        }
        if (me != null && by.containsKey(me.getHostAddress())) by.get(me.getHostAddress()).name = T.t("This phone");
        List<Survey.LanDevice> out = new ArrayList<>(by.values());
        out.sort((x, y) -> Long.compare(order(x.ip), order(y.ip)));
        return out;
    }

    private static Survey.LanDevice device(String ip) {
        Survey.LanDevice d = new Survey.LanDevice();
        d.ip = ip;
        return d;
    }

    private static long order(String ip) {
        long v = 0;
        for (String p : ip.split("\\.")) {
            try {
                v = v * 256 + Integer.parseInt(p);
            } catch (NumberFormatException e) {
                return Long.MAX_VALUE;
            }
        }
        return v;
    }

    /** Knocks on a few common ports of every address in the phone's /24, in parallel; names come from reverse DNS. */
    private static void sweep(Network net, Inet4Address me, Map<String, Survey.LanDevice> by) {
        byte[] base = me.getAddress();
        Map<String, Set<Integer>> open = new java.util.concurrent.ConcurrentHashMap<>();
        Set<String> alive = java.util.concurrent.ConcurrentHashMap.newKeySet();
        ExecutorService pool = Executors.newFixedThreadPool(96);
        for (int host = 1; host < 255; host++) {
            byte[] a = base.clone();
            a[3] = (byte) host;
            for (int port : Lan.PORTS) {
                pool.execute(() -> {
                    try {
                        InetAddress ip = InetAddress.getByAddress(a);
                        try (Socket s = net.getSocketFactory().createSocket()) {
                            s.connect(new InetSocketAddress(ip, port), 400);
                            alive.add(ip.getHostAddress());
                            open.computeIfAbsent(ip.getHostAddress(), k -> java.util.concurrent.ConcurrentHashMap.newKeySet()).add(port);
                        } catch (ConnectException e) {
                            // refused = someone is there
                            if (e.getMessage() != null && e.getMessage().contains("refused")) alive.add(ip.getHostAddress());
                        }
                    } catch (Exception ignored) {
                        // nobody, or filtered
                    }
                });
            }
        }
        pool.shutdown();
        try {
            pool.awaitTermination(15, TimeUnit.SECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
        ExecutorService names = Executors.newFixedThreadPool(16);
        Map<String, String> host = new java.util.concurrent.ConcurrentHashMap<>();
        for (String ip : alive) {
            names.execute(() -> {
                try {
                    String n = Lan.shortName(InetAddress.getByName(ip).getCanonicalHostName());
                    if (!n.isEmpty()) host.put(ip, n);
                } catch (Exception ignored) {
                    // no name
                }
            });
        }
        names.shutdown();
        try {
            names.awaitTermination(6, TimeUnit.SECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
        for (String ip : alive) {
            Survey.LanDevice d = by.computeIfAbsent(ip, Scope::device);
            if (d.name.isEmpty() && host.containsKey(ip)) d.name = host.get(ip);
            if (d.kind.isEmpty()) d.kind = Lan.kindOfPorts(open.getOrDefault(ip, new HashSet<>()));
        }
    }

    /** Resolves the mDNS services found to addresses, one at a time (older Android allows only one). */
    @SuppressWarnings("deprecation")
    private static void resolveAll(NsdManager nsd, ConcurrentLinkedQueue<NsdServiceInfo> found, Map<String, Survey.LanDevice> by) {
        long end = SystemClock.elapsedRealtime() + 6000;
        Set<String> done = new HashSet<>();
        for (NsdServiceInfo s : found) {
            if (SystemClock.elapsedRealtime() > end || !done.add(s.getServiceName() + s.getServiceType())) continue;
            final Object wait = new Object();
            final NsdServiceInfo[] got = new NsdServiceInfo[1];
            try {
                nsd.resolveService(s, new NsdManager.ResolveListener() {
                    @Override
                    public void onResolveFailed(NsdServiceInfo i, int e) {
                        synchronized (wait) {
                            wait.notifyAll();
                        }
                    }

                    @Override
                    public void onServiceResolved(NsdServiceInfo i) {
                        synchronized (wait) {
                            got[0] = i;
                            wait.notifyAll();
                        }
                    }
                });
                synchronized (wait) {
                    if (got[0] == null) wait.wait(1500);
                }
            } catch (Exception ignored) {
                continue;
            }
            NsdServiceInfo i = got[0];
            if (i == null || i.getHost() == null || i.getHost() instanceof Inet6Address) continue;
            Survey.LanDevice d = by.computeIfAbsent(i.getHost().getHostAddress(), Scope::device);
            // The mDNS name is the one people gave the device ("Living room TV").
            if (d.name.isEmpty() || !s.getServiceType().contains("_http")) d.name = i.getServiceName();
            String k = Lan.kindOfService(s.getServiceType());
            if (!k.isEmpty() && (d.kind.isEmpty() || d.kind.equals("Device with a web page"))) d.kind = k;
        }
    }
}
