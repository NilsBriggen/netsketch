package org.netsketch.survey;

import java.io.IOException;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.net.SocketTimeoutException;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * The home network's side, in plain Java: UPnP/SSDP (who answers, device descriptions, the router's
 * external address), the public address from Cloudflare's trace page, address kinds (private, CGNAT)
 * and what a device probably is from its services or open ports. The Android side (Scope.java) does
 * the sockets on the Wi-Fi; test/AirTest.java checks the parsing on a computer.
 */
public final class Lan {
    private Lan() {}

    public static final String SSDP_HOST = "239.255.255.250";
    public static final int SSDP_PORT = 1900;
    public static final String IGD = "urn:schemas-upnp-org:device:InternetGatewayDevice:1";
    /** Ports knocked on per address: liveness, and a hint at what the device is. */
    public static final int[] PORTS = {80, 443, 445, 22, 62078, 8009, 9100, 554};
    /** mDNS service types asked for. */
    public static final String[] SERVICES = {"_http._tcp", "_ipp._tcp", "_printer._tcp", "_googlecast._tcp", "_airplay._tcp", "_raop._tcp", "_hap._tcp", "_smb._tcp", "_spotify-connect._tcp"};

    public static String msearch(String st) {
        return "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: " + st + "\r\n\r\n";
    }

    /** One answer to an M-SEARCH. */
    public static final class Answer {
        public String from = "", location = "", st = "", server = "";
    }

    public static Answer parseAnswer(String text, String from) {
        if (text == null || !text.startsWith("HTTP/1.1 200")) return null;
        Answer a = new Answer();
        a.from = from;
        for (String l : text.split("\r?\n")) {
            int c = l.indexOf(':');
            if (c <= 0) continue;
            String k = l.substring(0, c).trim().toUpperCase(Locale.ROOT), v = l.substring(c + 1).trim();
            if (k.equals("LOCATION")) a.location = v;
            else if (k.equals("ST")) a.st = v;
            else if (k.equals("SERVER")) a.server = v;
        }
        return a.location.isEmpty() ? null : a;
    }

    /** Asks (twice, UDP can get lost) and collects answers for ms milliseconds, one per description address. */
    public static List<Answer> search(DatagramSocket s, String st, int ms) throws IOException {
        byte[] ask = msearch(st).getBytes(StandardCharsets.US_ASCII);
        InetAddress group = InetAddress.getByName(SSDP_HOST);
        s.send(new DatagramPacket(ask, ask.length, group, SSDP_PORT));
        s.send(new DatagramPacket(ask, ask.length, group, SSDP_PORT));
        Map<String, Answer> out = new LinkedHashMap<>();
        long end = System.currentTimeMillis() + ms;
        byte[] buf = new byte[2048];
        while (true) {
            long left = end - System.currentTimeMillis();
            if (left <= 0) break;
            s.setSoTimeout((int) left);
            DatagramPacket in = new DatagramPacket(buf, buf.length);
            try {
                s.receive(in);
            } catch (SocketTimeoutException e) {
                break;
            }
            Answer a = parseAnswer(new String(in.getData(), 0, in.getLength(), StandardCharsets.UTF_8), in.getAddress().getHostAddress());
            if (a != null) out.putIfAbsent(a.location, a);
        }
        return new ArrayList<>(out.values());
    }

    /** The text of the first <name> element (any namespace prefix), unescaped; "" if missing. */
    public static String tag(String xml, String name) {
        if (xml == null) return "";
        Matcher m = Pattern.compile("<(?:\\w+:)?" + Pattern.quote(name) + "\\b[^>]*>(.*?)</(?:\\w+:)?" + Pattern.quote(name) + ">", Pattern.DOTALL).matcher(xml);
        if (!m.find()) return "";
        return m.group(1).trim().replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
    }

    /** {control URL (absolute), service type} of the router's WANIPConnection / WANPPPConnection, or null. */
    public static String[] wanControl(String xml, String location) {
        if (xml == null) return null;
        Matcher m = Pattern.compile("<(?:\\w+:)?service>(.*?)</(?:\\w+:)?service>", Pattern.DOTALL).matcher(xml);
        String base = tag(xml, "URLBase");
        while (m.find()) {
            String svc = m.group(1), type = tag(svc, "serviceType");
            if (!type.contains("WANIPConnection") && !type.contains("WANPPPConnection")) continue;
            String url = resolve(base.isEmpty() ? location : base, tag(svc, "controlURL"));
            if (url != null) return new String[] {url, type};
        }
        return null;
    }

    public static String resolve(String base, String rel) {
        try {
            return java.net.URI.create(base).resolve(rel).toString();
        } catch (Exception e) {
            return null;
        }
    }

    public static String soapExternalIp(String serviceType) {
        return "<?xml version=\"1.0\"?>\r\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">"
                + "<s:Body><u:GetExternalIPAddress xmlns:u=\"" + serviceType + "\"></u:GetExternalIPAddress></s:Body></s:Envelope>";
    }

    /** The "ip=" line of https://speed.cloudflare.com/cdn-cgi/trace. */
    public static String traceIp(String trace) {
        if (trace == null) return null;
        for (String l : trace.split("\n")) if (l.startsWith("ip=")) return l.substring(3).trim();
        return null;
    }

    private static int[] v4(String ip) {
        if (ip == null) return null;
        String[] p = ip.trim().split("\\.");
        if (p.length != 4) return null;
        int[] b = new int[4];
        try {
            for (int i = 0; i < 4; i++) {
                b[i] = Integer.parseInt(p[i]);
                if (b[i] < 0 || b[i] > 255) return null;
            }
        } catch (NumberFormatException e) {
            return null;
        }
        return b;
    }

    public static boolean isV4(String ip) {
        return v4(ip) != null;
    }

    /** 100.64.0.0/10: the provider's shared address space (carrier-grade NAT). */
    public static boolean cgnat(String ip) {
        int[] b = v4(ip);
        return b != null && b[0] == 100 && b[1] >= 64 && b[1] <= 127;
    }

    /** 10/8, 172.16/12, 192.168/16: a home network's own addresses. */
    public static boolean isPrivate(String ip) {
        int[] b = v4(ip);
        return b != null && (b[0] == 10 || (b[0] == 172 && b[1] >= 16 && b[1] <= 31) || (b[0] == 192 && b[1] == 168));
    }

    /** What a device is from an mDNS service type ("" = no idea). English; shown through T. */
    public static String kindOfService(String type) {
        String t = type == null ? "" : type.toLowerCase(Locale.ROOT);
        if (t.contains("_ipp") || t.contains("_printer")) return "Printer";
        if (t.contains("_googlecast")) return "TV or speaker (Chromecast)";
        if (t.contains("_airplay")) return "Apple TV or AirPlay device";
        if (t.contains("_raop")) return "AirPlay speaker";
        if (t.contains("_hap")) return "Smart-home device";
        if (t.contains("_smb")) return "Computer or NAS";
        if (t.contains("_spotify-connect")) return "Speaker";
        if (t.contains("_http")) return "Device with a web page";
        return "";
    }

    /** What a device is from its UPnP device type. */
    public static String kindOfUpnp(String deviceType) {
        String t = deviceType == null ? "" : deviceType;
        if (t.contains("InternetGatewayDevice")) return "Router";
        if (t.contains("WFADevice")) return "Access point";
        if (t.contains("MediaRenderer")) return "TV or speaker";
        if (t.contains("MediaServer")) return "Media server";
        if (t.contains("Printer")) return "Printer";
        return "";
    }

    /** What a device probably is from the ports it answered on. */
    public static String kindOfPorts(Set<Integer> open) {
        if (open.contains(62078)) return "iPhone or iPad";
        if (open.contains(8009)) return "TV or speaker (Chromecast)";
        if (open.contains(9100)) return "Printer";
        if (open.contains(554)) return "Camera";
        if (open.contains(445)) return "Computer or NAS";
        if (open.contains(22)) return "Computer or NAS";
        if (open.contains(80) || open.contains(443)) return "Device with a web page";
        return "";
    }

    /** "Galaxy-S23.fritz.box" → "Galaxy-S23"; an address stays as it is. */
    public static String shortName(String host) {
        if (host == null || host.isEmpty() || isV4(host) || host.contains(":")) return "";
        int dot = host.indexOf('.');
        return dot > 0 ? host.substring(0, dot) : host;
    }
}
