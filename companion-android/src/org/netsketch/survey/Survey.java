package org.netsketch.survey;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

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
                b.append(",\n     \"speed\": {\"down_mbps\": ").append(num(p.speed.downMbps)).append(", \"latency_ms\": ").append(num(p.speed.latencyMs)).append("}");
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
}
