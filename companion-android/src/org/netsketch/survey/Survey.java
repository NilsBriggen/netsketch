package org.netsketch.survey;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;

/**
 * The measurements, and the file NetSketch imports (".netsurvey", JSON). Plain Java on purpose:
 * no Android classes, so it can be checked on a computer (see test/SurveyJsonTest.java).
 */
public final class Survey {
    public static final String FORMAT = "netsketch-survey";

    /** One network heard at a point (averaged over the scans there). */
    public static final class Seen {
        public String ssid = "", bssid = "", security = "", standard = "";
        public int freq, channel, width = 20;
        public double rssi;
        public Integer stations;
        public Double utilization;
        /** How many scans heard it (for the average). */
        public transient int count;
        public transient double sum;
    }

    /** The phone's own Wi-Fi connection at a point. */
    public static final class Connected {
        public String ssid = "", bssid = "";
        public double rssi, linkMbps;
        public int freq;
    }

    public static final class Speed {
        public double downMbps, latencyMs;
        /** 0 = not measured (files from app 1.0). */
        public double upMbps, jitterMs;
    }

    public static final class Point {
        public String name = "", time = "";
        public int floor, scans;
        /** Where on the floor plan (metres) it was taken, if the person tapped it; and whether NetSketch has it. */
        public Double x, y;
        public boolean sent;
        public Connected connected;
        public Speed speed;
        public final List<Seen> networks = new ArrayList<>();
    }

    public String device = "";
    public final List<Point> points = new ArrayList<>();

    /** Keeps a new point; refuses one that heard no network at all (false): it would only be an empty pin. */
    public boolean add(Point p) {
        if (p.networks.isEmpty()) return false;
        points.add(p);
        return true;
    }

    // ---------- file ----------

    /** A file with just one point (what the live link sends after each measurement). */
    public String toJson(Point only) {
        Survey one = new Survey();
        one.device = device;
        one.points.add(only);
        return one.toJson();
    }

    public String toJson() {
        StringBuilder b = new StringBuilder();
        b.append("{\n  \"format\": ").append(q(FORMAT)).append(",\n  \"version\": 1,\n  \"device\": ").append(q(device)).append(",\n  \"points\": [");
        for (int i = 0; i < points.size(); i++) {
            Point p = points.get(i);
            b.append(i == 0 ? "\n" : ",\n").append("    {\"name\": ").append(q(p.name)).append(", \"floor\": ").append(p.floor)
                    .append(", \"time\": ").append(q(p.time)).append(", \"scans\": ").append(p.scans);
            if (p.x != null && p.y != null) b.append(", \"x\": ").append(num(p.x)).append(", \"y\": ").append(num(p.y));
            if (p.sent) b.append(", \"sent\": true");
            if (p.connected != null) {
                Connected c = p.connected;
                b.append(",\n     \"connected\": {\"ssid\": ").append(q(c.ssid)).append(", \"bssid\": ").append(q(c.bssid))
                        .append(", \"rssi\": ").append(num(c.rssi)).append(", \"link_mbps\": ").append(num(c.linkMbps)).append(", \"freq\": ").append(c.freq).append("}");
            }
            if (p.speed != null) {
                b.append(",\n     \"speed\": {\"down_mbps\": ").append(num(p.speed.downMbps)).append(", \"latency_ms\": ").append(num(p.speed.latencyMs));
                if (p.speed.upMbps > 0) b.append(", \"up_mbps\": ").append(num(p.speed.upMbps)).append(", \"jitter_ms\": ").append(num(p.speed.jitterMs));
                b.append("}");
            }
            b.append(",\n     \"networks\": [");
            for (int j = 0; j < p.networks.size(); j++) {
                Seen s = p.networks.get(j);
                b.append(j == 0 ? "\n" : ",\n").append("      {\"ssid\": ").append(q(s.ssid)).append(", \"bssid\": ").append(q(s.bssid))
                        .append(", \"freq\": ").append(s.freq).append(", \"channel\": ").append(s.channel).append(", \"width\": ").append(s.width)
                        .append(", \"rssi\": ").append(num(s.rssi)).append(", \"security\": ").append(q(s.security)).append(", \"standard\": ").append(q(s.standard));
                if (s.stations != null) b.append(", \"stations\": ").append(s.stations);
                if (s.utilization != null) b.append(", \"utilization\": ").append(num(s.utilization));
                b.append("}");
            }
            b.append(p.networks.isEmpty() ? "]}" : "\n     ]}");
        }
        b.append(points.isEmpty() ? "]\n}\n" : "\n  ]\n}\n");
        return b.toString();
    }

    static String num(double v) {
        return String.format(Locale.ROOT, "%.2f", v);
    }

    /** A JSON string, escaped. */
    public static String q(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (char c : (s == null ? "" : s).toCharArray()) {
            switch (c) {
                case '"': b.append("\\\""); break;
                case '\\': b.append("\\\\"); break;
                case '\n': b.append("\\n"); break;
                case '\r': b.append("\\r"); break;
                case '\t': b.append("\\t"); break;
                default:
                    if (c < 0x20) b.append(String.format(Locale.ROOT, "\\u%04x", (int) c));
                    else b.append(c);
            }
        }
        return b.append('"').toString();
    }

    // ---------- reading the air ----------

    /** Channel number from the centre frequency (MHz). */
    public static int channelOf(int freq) {
        if (freq == 2484) return 14;
        if (freq >= 2412 && freq < 2484) return (freq - 2407) / 5;
        if (freq >= 5955 && freq <= 7115) return (freq - 5950) / 5;
        if (freq >= 5160 && freq <= 5885) return (freq - 5000) / 5;
        return 0;
    }

    /** The MHz a band's channel chart spans: every 20 MHz channel, up to 5 GHz ch 177 and 6 GHz ch 233. */
    public static int[] chartSpan(String band) {
        switch (band) {
            case "2.4 GHz": return new int[] {2400, 2496};
            case "5 GHz": return new int[] {5160, 5895};
            default: return new int[] {5935, 7125};
        }
    }

    public static String bandOf(int freq) {
        return freq < 3000 ? "2.4 GHz" : freq < 5925 ? "5 GHz" : "6 GHz";
    }

    /** Android's channel-width constants (ScanResult.CHANNEL_WIDTH_*) in MHz. */
    public static int widthOf(int c) {
        switch (c) {
            case 1: return 40;
            case 2: return 80;
            case 3: case 4: return 160;
            case 5: return 320;
            default: return 20;
        }
    }

    /** Android's Wi-Fi standard constants (ScanResult.WIFI_STANDARD_*). */
    public static String standardOf(int s) {
        switch (s) {
            case 4: return "Wi-Fi 4";
            case 5: return "Wi-Fi 5";
            case 6: return "Wi-Fi 6";
            case 8: return "Wi-Fi 7";
            default: return "";
        }
    }

    /** Security from the scan's capability string, e.g. "[WPA2-PSK-CCMP][RSN-SAE-CCMP][ESS]". */
    public static String securityOf(String caps) {
        if (caps == null) return "";
        if (caps.contains("EAP")) return "WPA Enterprise";
        if (caps.contains("SAE") && caps.contains("PSK")) return "WPA2/WPA3";
        if (caps.contains("SAE")) return "WPA3";
        if (caps.contains("PSK")) return "WPA2";
        if (caps.contains("WEP")) return "WEP";
        if (caps.contains("OWE")) return "Enhanced Open";
        return "Open";
    }

    /**
     * The access point's "BSS load" announcement (information element 11): how many devices are on it
     * and how busy the channel is. Returns {stations, utilization 0..1} or null.
     */
    public static double[] bssLoad(byte[] ie) {
        if (ie == null || ie.length < 3) return null;
        int stations = (ie[0] & 0xff) | ((ie[1] & 0xff) << 8);
        return new double[] {stations, (ie[2] & 0xff) / 255.0};
    }

    // ---------- what a point shows ----------

    /** Whether a network heard at a point is one of the person's own (same name as the plan's or the phone's). */
    public static boolean isOwn(Point p, Seen s, Set<String> mine) {
        if (p.connected != null && s.bssid.equalsIgnoreCase(p.connected.bssid)) return true;
        if (s.ssid.isEmpty()) return false;
        return mine.contains(s.ssid) || (p.connected != null && s.ssid.equals(p.connected.ssid));
    }

    /** Overlapping 2.4 GHz channels: 20 MHz wide, 5 MHz apart, so closer than 5 numbers collide. */
    static boolean overlaps24(int a, int b) {
        return Math.abs(a - b) < 5;
    }

    /**
     * Plain-language findings for one point, worst first: weak signal, a phone sticking to a far access
     * point, the slow band, crowded or in-between 2.4 GHz channels, a busy channel, open or WEP networks.
     */
    public static List<String> advice(Point p, Set<String> mine) {
        Set<String> out = new LinkedHashSet<>();
        Connected c = p.connected;
        double best = -200, best5 = -200;
        for (Seen s : p.networks) {
            if (!isOwn(p, s, mine)) continue;
            best = Math.max(best, s.rssi);
            if (s.freq >= 5000) best5 = Math.max(best5, s.rssi);
        }
        double here = c != null ? c.rssi : best;
        if (here > -200 && here < -70) out.add(T.f("Weak signal here (%.0f dBm): expect slow speeds and dropouts. An access point closer to this spot, or a cable, helps.", here));
        if (c != null) {
            double other = -200;
            for (Seen s : p.networks) {
                if (isOwn(p, s, mine) && !s.bssid.equalsIgnoreCase(c.bssid) && !sameRadio(s.bssid, c.bssid)) other = Math.max(other, s.rssi);
            }
            if (c.rssi < -65 && other - c.rssi >= 10) out.add(T.f("The phone stays on an access point at %.0f dBm although another one of yours offers %.0f dBm here (a 'sticky client'). Fast roaming (802.11k/v/r) helps devices switch in time.", c.rssi, other));
            if (c.freq < 3000 && best5 >= -67) out.add(T.f("The phone uses 2.4 GHz although your 5 GHz network is good here (%.0f dBm). Band steering, or one name for both bands, moves devices to the faster band.", best5));
        }
        for (Seen s : p.networks) {
            if (!isOwn(p, s, mine) || s.freq >= 3000 || s.channel < 1 || s.channel > 13) continue;
            int crowd = 0;
            for (Seen o : p.networks) if (!isOwn(p, o, mine) && o.freq < 3000 && o.rssi >= -75 && overlaps24(o.channel, s.channel)) crowd++;
            if (crowd >= 3) {
                int pick = s.channel, least = Integer.MAX_VALUE;
                for (int ch : new int[] {1, 6, 11}) {
                    int n = 0;
                    for (Seen o : p.networks) if (!isOwn(p, o, mine) && o.freq < 3000 && o.rssi >= -75 && overlaps24(o.channel, ch)) n++;
                    if (n < least) {
                        least = n;
                        pick = ch;
                    }
                }
                if (pick != s.channel && least < crowd) out.add(T.f("Your 2.4 GHz channel %d is shared with %d other networks heard clearly here. Try channel %d.", s.channel, crowd, pick));
            }
            if (s.channel != 1 && s.channel != 5 && s.channel != 6 && s.channel != 9 && s.channel != 11 && s.channel != 13) {
                out.add(T.f("Channel %d sits between the non-overlapping ones (1, 6, 11 — or 1, 5, 9, 13 in Europe) and collides with neighbours on both sides.", s.channel));
            }
        }
        for (Seen s : p.networks) {
            if (c != null && s.bssid.equalsIgnoreCase(c.bssid) && s.utilization != null && s.utilization > 0.5) {
                out.add(T.f("Your access point reports its channel %.0f%% busy: others take turns on it too.", s.utilization * 100));
            }
            if (isOwn(p, s, mine) && s.security.equals("Open")) out.add(T.f("'%s' is open (no password): anyone nearby can join it.", s.ssid));
            if (isOwn(p, s, mine) && s.security.equals("WEP")) out.add(T.f("'%s' still uses WEP, which is cracked in minutes. Switch it to WPA3.", s.ssid));
        }
        if (out.isEmpty() && here >= -65) out.add(T.t("Good spot: strong signal and no crowded channel."));
        return new ArrayList<>(out);
    }

    /** Two radios of one access point usually differ only in the last digits of their address. */
    public static boolean sameRadio(String a, String b) {
        return a.length() == 17 && b.length() == 17 && a.regionMatches(true, 0, b, 0, 15);
    }
}
