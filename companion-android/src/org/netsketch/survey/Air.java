package org.netsketch.survey;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * What the air around the phone says beyond the basics: the extra announcements in each scan (country,
 * spatial streams, fast roaming 802.11k/v/r, WPS, BSS load), which radios belong to one box, how wide a
 * channel really spans, and every access point heard over time (signal history, channel changes).
 * Plain Java, so test/AirTest.java checks it on a computer.
 */
public final class Air {
    private Air() {}

    /** Readings kept per access point for its little history line. */
    public static final int HISTORY = 40;

    /** Everything heard since the app started (shared by the screen and the watch service). */
    public static final Tracker TRACKER = new Tracker();

    // ---------- information elements ----------

    /** Reads one information element of a scan into s: its id, the extension id (for id 255, else -1) and its bytes. */
    public static void read(Survey.Seen s, int id, int ext, byte[] b) {
        if (b == null) return;
        s.ies = true;
        switch (id) {
            case 7: // Country: two letters, then the environment
                if (b.length >= 2 && Character.isLetter(b[0]) && Character.isLetter(b[1])) s.country = ("" + (char) b[0] + (char) b[1]).toUpperCase(Locale.ROOT);
                break;
            case 11: // BSS load
                double[] load = Survey.bssLoad(b);
                if (load != null) {
                    s.stations = (int) load[0];
                    s.utilization = load[1];
                }
                break;
            case 45: // HT capabilities: the receive MCS bitmask (one byte per stream) starts at byte 3
                if (b.length >= 7) {
                    int n = 0;
                    for (int i = 3; i < 7; i++) if (b[i] != 0) n = i - 2;
                    s.streams = Math.max(s.streams, n);
                }
                break;
            case 191: // VHT capabilities: receive MCS map at bytes 4-5
                if (b.length >= 6) s.streams = Math.max(s.streams, streams(b[4], b[5]));
                break;
            case 54: // Mobility Domain: fast transition (802.11r)
                s.r = true;
                break;
            case 70: // RM Enabled Capabilities: bit 1 = neighbor reports (802.11k)
                if (b.length >= 1 && (b[0] & 0x02) != 0) s.k = true;
                break;
            case 127: // Extended Capabilities: bit 19 = BSS transition (802.11v)
                if (b.length >= 3 && (b[2] & 0x08) != 0) s.v = true;
                break;
            case 221: // Vendor specific: Microsoft OUI 00:50:F2 type 4 = WPS
                if (b.length >= 4 && b[0] == 0 && (b[1] & 0xff) == 0x50 && (b[2] & 0xff) == 0xf2 && b[3] == 4) s.wps = true;
                break;
            case 255: // Extension 35 = HE capabilities: 6 MAC + 11 PHY bytes, then the receive MCS map (<= 80 MHz)
                if (ext == 35 && b.length >= 19) s.streams = Math.max(s.streams, streams(b[17], b[18]));
                break;
            default:
                break;
        }
    }

    /** An MCS map: 2 bits per stream, 3 = not supported. The highest supported stream count. */
    static int streams(byte lo, byte hi) {
        int map = (lo & 0xff) | ((hi & 0xff) << 8), n = 0;
        for (int i = 0; i < 8; i++) if (((map >> (2 * i)) & 3) != 3) n = i + 1;
        return n;
    }

    // ---------- channels ----------

    /** The MHz a network really covers {low, high}: the scan reports the primary 20 MHz of a wider channel. */
    public static int[] span(int freq, int width) {
        double centre = freq, half = width / 2.0;
        if (width > 20 && freq >= 5000) {
            int base = freq >= 5925 ? 5955 : 5180;
            centre = base - 10 + Math.floor((freq - (base - 10)) / (double) width) * width + half;
        } else if (width == 40 && freq < 3000) {
            centre = freq + 10; // shortcut: assumes the second half above; the scan doesn't say which side
        }
        return new int[] {(int) Math.round(centre - half), (int) Math.round(centre + half)};
    }

    public static boolean overlap(int[] a, int[] b) {
        return a[0] < b[1] && b[0] < a[1];
    }

    public static int freqOf(int channel, String band) {
        if (band.equals("2.4 GHz")) return channel == 14 ? 2484 : 2407 + 5 * channel;
        return (band.equals("5 GHz") ? 5000 : 5950) + 5 * channel;
    }

    /** 5 GHz channels that can jump away when they hear radar (DFS). */
    public static boolean dfs(int channel) {
        return channel >= 52 && channel <= 144;
    }

    // ---------- which radios are one box ----------

    /**
     * Radios of one box usually share the first five bytes of their address, or differ only in the
     * "locally administered" bit of the first byte (the box's extra networks). Same key = same box.
     */
    public static String boxKey(String bssid) {
        String[] p = bssid.toLowerCase(Locale.ROOT).split(":");
        if (p.length != 6) return bssid.toLowerCase(Locale.ROOT);
        try {
            return String.format(Locale.ROOT, "%02x:%s:%s:%s:%s", Integer.parseInt(p[0], 16) & ~0x02, p[1], p[2], p[3], p[4]);
        } catch (NumberFormatException e) {
            return bssid.toLowerCase(Locale.ROOT);
        }
    }

    /** "HomeNet (aa:bb:c0)": a network name and the end of its address, to tell boxes apart. */
    public static String label(String ssid, String bssid) {
        String end = bssid.length() >= 8 ? bssid.substring(bssid.length() - 8).toLowerCase(Locale.ROOT) : bssid;
        return (ssid.isEmpty() ? "?" : ssid) + " (" + end + ")";
    }

    /** For an extender that got its own name ("HomeNet_EXT"), the name it extends; else null. */
    public static String extBase(String ssid, Collection<String> names) {
        for (String n : names) {
            if (n.isEmpty() || ssid.length() <= n.length() || !ssid.startsWith(n)) continue;
            if (ssid.substring(n.length()).matches("(?i)[-_ .]?(ext|extender|repeater|rpt|re|booster|plus)[0-9]*")) return n;
        }
        return null;
    }

    // ---------- every access point heard ----------

    /** One radio (BSSID) heard over time. */
    public static final class Ap {
        /** The newest reading. */
        public Survey.Seen last;
        public final float[] rssi = new float[HISTORY];
        public final long[] at = new long[HISTORY];
        public int n;
        long lastTs = -1;
        public Double distanceM;

        public static Ap of(Survey.Seen s) {
            Ap a = new Ap();
            a.last = s;
            a.add(s.at, s.rssi);
            return a;
        }

        void add(long t, double r) {
            if (n == HISTORY) {
                System.arraycopy(rssi, 1, rssi, 0, n - 1);
                System.arraycopy(at, 1, at, 0, n - 1);
                n--;
            }
            rssi[n] = (float) r;
            at[n++] = t;
        }

        public float[] history() {
            return Arrays.copyOf(rssi, n);
        }

        public long lastSeen() {
            return n == 0 ? 0 : at[n - 1];
        }

        /** {mean signal, readings} of the readings at or after t. */
        public double[] since(long t) {
            double sum = 0;
            int k = 0;
            for (int i = 0; i < n; i++) {
                if (at[i] < t) continue;
                sum += rssi[i];
                k++;
            }
            return new double[] {k == 0 ? Double.NaN : sum / k, k};
        }
    }

    /** The radios of one physical access point (router, extender, mesh node). */
    public static final class Box {
        public final String key;
        public final List<Ap> radios = new ArrayList<>();
        public boolean own;

        Box(String key) {
            this.key = key;
        }

        public double best() {
            double b = -200;
            for (Ap a : radios) b = Math.max(b, a.last.rssi);
            return b;
        }

        public Set<String> names() {
            Set<String> s = new LinkedHashSet<>();
            for (Ap a : radios) if (!a.last.ssid.isEmpty()) s.add(a.last.ssid);
            return s;
        }

        public Set<String> bands() {
            Set<String> s = new LinkedHashSet<>();
            for (Ap a : radios) s.add(Survey.bandOf(a.last.freq));
            return s;
        }
    }

    /** The person's own network names: the plan's, the connected one, and extenders named after them. */
    public static Set<String> ownNames(Set<String> mine, String connectedSsid) {
        Set<String> n = new HashSet<>(mine);
        if (connectedSsid != null && !connectedSsid.isEmpty()) n.add(connectedSsid);
        return n;
    }

    /** Groups radios into boxes; a box is yours if any of its radios carries one of your names (or is the connected one). */
    public static List<Box> group(Collection<Ap> aps, Set<String> mine, String connectedSsid, String connectedBssid) {
        Set<String> names = ownNames(mine, connectedSsid);
        Map<String, Box> by = new LinkedHashMap<>();
        for (Ap a : aps) {
            Box b = by.computeIfAbsent(boxKey(a.last.bssid), Box::new);
            b.radios.add(a);
            String id = a.last.ssid;
            if (names.contains(id) || a.last.bssid.equalsIgnoreCase(connectedBssid == null ? "" : connectedBssid) || (!id.isEmpty() && extBase(id, names) != null)) b.own = true;
        }
        List<Box> out = new ArrayList<>(by.values());
        for (Box b : out) b.radios.sort((x, y) -> Integer.compare(x.last.freq, y.last.freq));
        out.sort((x, y) -> x.own != y.own ? (x.own ? -1 : 1) : Double.compare(y.best(), x.best()));
        return out;
    }

    /** Keeps every access point heard, its signal over time, and notices when one changes channel. */
    public static final class Tracker {
        public final Map<String, Ap> aps = new LinkedHashMap<>();
        /** Channel changes seen so far (also handed to the watch log while watching). */
        public final List<Survey.Event> changes = new ArrayList<>();

        /** Adds one scan's results; returns the channel changes it shows. A reading already seen (same timestamp) is skipped. */
        public synchronized List<Survey.Event> update(Collection<Survey.Seen> seen) {
            List<Survey.Event> out = new ArrayList<>();
            for (Survey.Seen s : seen) {
                Ap a = aps.computeIfAbsent(s.bssid.toLowerCase(Locale.ROOT), k -> new Ap());
                if (a.last != null && s.tsUs > 0 && s.tsUs <= a.lastTs) continue;
                if (a.last != null && s.channel > 0 && a.last.channel > 0 && s.channel != a.last.channel && s.at >= a.lastSeen()) {
                    Survey.Event e = new Survey.Event();
                    e.at = s.at;
                    e.kind = "channel";
                    e.bssid = s.bssid;
                    e.ssid = s.ssid;
                    e.from = String.valueOf(a.last.channel);
                    e.to = String.valueOf(s.channel);
                    e.value = s.rssi;
                    e.note = Survey.bandOf(s.freq);
                    out.add(e);
                }
                if (s.distanceM == null && a.distanceM != null) s.distanceM = a.distanceM;
                a.last = s;
                a.lastTs = s.tsUs;
                a.add(s.at, s.rssi);
            }
            changes.addAll(out);
            return out;
        }

        /** A Wi-Fi RTT distance for one radio. */
        public synchronized void distance(String bssid, double m) {
            Ap a = aps.get(bssid.toLowerCase(Locale.ROOT));
            if (a == null) return;
            a.distanceM = Math.max(0, m);
            a.last.distanceM = a.distanceM;
        }

        /** The radios heard at or after t. */
        public synchronized List<Ap> since(long t) {
            List<Ap> out = new ArrayList<>();
            for (Ap a : aps.values()) if (a.lastSeen() >= t) out.add(a);
            return out;
        }

        public synchronized List<Survey.Event> changes() {
            return new ArrayList<>(changes);
        }
    }
}
