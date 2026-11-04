import org.netsketch.survey.Survey;
import org.netsketch.survey.T;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Checks the plain-Java parts of the companion app on a computer, and writes sample.netsurvey —
 * the file NetSketch's own test imports, so both sides always agree on the format.
 * Run by build.sh.
 */
public class SurveyJsonTest {
    static void check(boolean ok, String what) {
        if (!ok) throw new AssertionError(what);
    }

    static Survey.Seen seen(String ssid, String bssid, int freq, int width, double rssi, String sec, String std) {
        Survey.Seen s = new Survey.Seen();
        s.ssid = ssid;
        s.bssid = bssid;
        s.freq = freq;
        s.channel = Survey.channelOf(freq);
        s.width = width;
        s.rssi = rssi;
        s.security = sec;
        s.standard = std;
        return s;
    }

    static java.util.List<String> placeholders(Pattern p, String s) {
        java.util.List<String> out = new java.util.ArrayList<>();
        Matcher m = p.matcher(s);
        while (m.find()) out.add(m.group());
        return out;
    }

    public static void main(String[] args) throws Exception {
        check(Survey.channelOf(2437) == 6 && Survey.channelOf(2484) == 14 && Survey.channelOf(5220) == 44 && Survey.channelOf(6115) == 33, "channels");
        check(Survey.securityOf("[WPA2-PSK-CCMP][RSN-SAE-CCMP][ESS]").equals("WPA2/WPA3") && Survey.securityOf("[ESS]").equals("Open") && Survey.securityOf("[WPA2-EAP-CCMP]").equals("WPA Enterprise"), "security");
        check(Survey.widthOf(2) == 80 && Survey.widthOf(0) == 20 && Survey.standardOf(8).equals("Wi-Fi 7"), "width/standard");
        double[] load = Survey.bssLoad(new byte[] {5, 0, (byte) 107, 0, 0});
        check(load[0] == 5 && Math.abs(load[1] - 107 / 255.0) < 1e-9 && Survey.bssLoad(new byte[] {1}) == null, "bss load");
        check(Survey.q("a\"b\\c\n").equals("\"a\\\"b\\\\c\\n\""), "escaping");
        // The channel chart reaches the top channels (5 GHz 177, 6 GHz 233), each with its 20 MHz.
        check(Survey.channelOf(5885) == 177 && Survey.chartSpan("5 GHz")[1] >= 5885 + 10 && Survey.chartSpan("5 GHz")[0] <= 5180 - 10, "5 GHz chart");
        check(Survey.channelOf(7115) == 233 && Survey.chartSpan("6 GHz")[1] >= 7115 + 10 && Survey.chartSpan("6 GHz")[0] <= 5955 - 10, "6 GHz chart");
        check(Survey.chartSpan("2.4 GHz")[1] >= 2484 + 10, "2.4 GHz chart");

        // A walk through the small flat: bedroom desk, sofa, hall.
        Survey s = new Survey();
        s.device = "Example phone (Android 15)";
        String[] names = {"Bedroom desk", "Sofa", "Hall"};
        double[][] nb = {{-56, -69}, {-64, -80}, {-68, -86}};
        double[][] own = {{-47, -58}, {-35, -41}, {-21, -24}};
        for (int i = 0; i < 3; i++) {
            Survey.Point p = new Survey.Point();
            p.name = names[i];
            p.time = "2026-11-04T10:0" + i + ":00Z";
            p.scans = 3;
            p.networks.add(seen("HomeNet", "d4:5d:64:aa:bb:c0", 2412, 20, own[i][0], "WPA2/WPA3", "Wi-Fi 6"));
            p.networks.add(seen("HomeNet", "d4:5d:64:aa:bb:c4", 5180, 80, own[i][1], "WPA2/WPA3", "Wi-Fi 6"));
            Survey.Seen a = seen("Flat-3B", "3c:a6:2f:11:22:30", 2437, 20, nb[i][0], "WPA2", "Wi-Fi 6");
            a.stations = 5;
            a.utilization = 0.42;
            p.networks.add(a);
            Survey.Seen b = seen("Flat-3B", "3c:a6:2f:11:22:34", 5220, 80, nb[i][1], "WPA2", "Wi-Fi 6");
            b.stations = 2;
            b.utilization = 0.18;
            p.networks.add(b);
            if (i == 0) p.networks.add(seen("Cafe \"Corner\"", "02:11:22:33:44:55", 2462, 20, -84, "Open", "Wi-Fi 4"));
            if (i == 1) {
                p.networks.add(seen("", "8e:00:11:22:33:44", 2437, 20, -78, "WPA2", ""));
                Survey.Connected c = new Survey.Connected();
                c.ssid = "HomeNet";
                c.bssid = "d4:5d:64:aa:bb:c4";
                c.rssi = -41;
                c.linkMbps = 864;
                c.freq = 5180;
                p.connected = c;
                Survey.Speed sp = new Survey.Speed();
                sp.downMbps = 312.5;
                sp.latencyMs = 14.2;
                sp.upMbps = 48.5;
                sp.jitterMs = 2.1;
                p.speed = sp;
            }
            s.points.add(p);
        }
        // A point that heard nothing isn't kept.
        check(!s.add(new Survey.Point()) && s.points.size() == 3, "empty point refused");
        String json = s.toJson();
        check(json.contains("\"format\": \"netsketch-survey\"") && json.contains("\"utilization\": 0.42") && json.contains("Cafe \\\"Corner\\\""), "json");
        check(new Survey().toJson().contains("\"points\": []"), "empty survey");
        check(json.contains("\"up_mbps\": 48.50, \"jitter_ms\": 2.10"), "upload and jitter written");

        // Every German text keeps the English one's placeholders, in the same order.
        Pattern ph = Pattern.compile("%[-0-9.]*[sdf%]");
        for (Map.Entry<String, String> e : T.DE.entrySet()) {
            check(placeholders(ph, e.getKey()).equals(placeholders(ph, e.getValue())), "placeholders: " + e.getKey());
        }
        T.german = true;
        check(T.f("Spot %d", 3).equals("Punkt 3") && T.t("not in the table").equals("not in the table"), "german lookup");
        java.util.Locale was = java.util.Locale.getDefault();
        java.util.Locale.setDefault(java.util.Locale.GERMANY);
        check(T.f("%.1f m", 3.2).equals("3,2 m"), "german numbers");
        java.util.Locale.setDefault(was);
        T.german = false;
        check(T.f("%.1f m", 3.2).equals("3.2 m"), "english numbers");

        // What a point shows. The sofa: the phone on the 5 GHz radio, nothing wrong.
        Set<String> mine = Set.of("HomeNet");
        check(s.points.get(1).networks.stream().filter(x -> Survey.isOwn(s.points.get(1), x, mine)).count() == 2, "own networks by name");
        check(Survey.advice(s.points.get(1), mine).size() == 1 && Survey.advice(s.points.get(1), mine).get(0).startsWith("Good spot"), "good spot");
        // A far corner: weak, stuck on a far access point, on 2.4 GHz channel 3 among three neighbours.
        Survey.Point far = new Survey.Point();
        far.networks.add(seen("HomeNet", "d4:5d:64:aa:bb:c0", 2422, 20, -78, "WPA2/WPA3", ""));
        far.networks.add(seen("HomeNet", "a0:00:00:00:00:04", 5180, 80, -60, "WPA2/WPA3", ""));
        for (int i = 0; i < 3; i++) far.networks.add(seen("N" + i, "0" + i + ":11:22:33:44:55", 2412 + 5 * i, 20, -60, "WPA2", ""));
        far.networks.add(seen("Old", "09:11:22:33:44:55", 2462, 20, -80, "WEP", ""));
        Survey.Connected fc = new Survey.Connected();
        fc.ssid = "HomeNet";
        fc.bssid = "d4:5d:64:aa:bb:c0";
        fc.rssi = -78;
        fc.freq = 2422;
        far.connected = fc;
        String all = String.join("\n", Survey.advice(far, mine));
        check(all.contains("Weak signal") && all.contains("sticky client") && all.contains("uses 2.4 GHz") && all.contains("Try channel 11") && all.contains("Channel 3 sits between"), "far corner advice:\n" + all);
        check(!all.contains("WEP"), "a neighbour's WEP isn't the person's problem");
        check(Survey.sameRadio("d4:5d:64:aa:bb:c0", "D4:5D:64:AA:BB:C4") && !Survey.sameRadio("d4:5d:64:aa:bb:c0", "a0:00:00:00:00:04"), "radios of one access point");
        // A point placed on the plan and already sent keeps both in the file.
        Survey.Point placed = new Survey.Point();
        placed.name = "Desk";
        placed.x = 2.5;
        placed.y = 1.25;
        placed.sent = true;
        String one = s.toJson(placed);
        check(one.contains("\"x\": 2.50, \"y\": 1.25") && one.contains("\"sent\": true") && one.contains("\"Desk\"") && !one.contains("Bedroom desk"), "single placed point");
        // The live link: NetSketch's answer and typed addresses.
        org.netsketch.survey.Link l = org.netsketch.survey.Link.parseAnswer("NETSKETCH 47878 Small flat", "192.168.1.23");
        check(l != null && l.port == 47878 && l.title.equals("Small flat") && l.host.equals("192.168.1.23"), "discovery answer");
        check(org.netsketch.survey.Link.parseAnswer("HELLO", "x") == null, "foreign answer ignored");
        check(org.netsketch.survey.Link.parseAddress("10.0.0.5").port == 47878 && org.netsketch.survey.Link.parseAddress("10.0.0.5:9000").port == 9000, "typed address");
        Path out = Path.of(args.length > 0 ? args[0] : "sample.netsurvey");
        Files.writeString(out, json);
        System.out.println("companion checks passed; wrote " + out);
    }
}
