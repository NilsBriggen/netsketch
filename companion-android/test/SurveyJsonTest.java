import org.netsketch.survey.Survey;

import java.nio.file.Files;
import java.nio.file.Path;

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

    public static void main(String[] args) throws Exception {
        check(Survey.channelOf(2437) == 6 && Survey.channelOf(2484) == 14 && Survey.channelOf(5220) == 44 && Survey.channelOf(6115) == 33, "channels");
        check(Survey.securityOf("[WPA2-PSK-CCMP][RSN-SAE-CCMP][ESS]").equals("WPA2/WPA3") && Survey.securityOf("[ESS]").equals("Open") && Survey.securityOf("[WPA2-EAP-CCMP]").equals("WPA Enterprise"), "security");
        check(Survey.widthOf(2) == 80 && Survey.widthOf(0) == 20 && Survey.standardOf(8).equals("Wi-Fi 7"), "width/standard");
        double[] load = Survey.bssLoad(new byte[] {5, 0, (byte) 107, 0, 0});
        check(load[0] == 5 && Math.abs(load[1] - 107 / 255.0) < 1e-9 && Survey.bssLoad(new byte[] {1}) == null, "bss load");
        check(Survey.q("a\"b\\c\n").equals("\"a\\\"b\\\\c\\n\""), "escaping");

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
                p.speed = sp;
            }
            s.points.add(p);
        }
        String json = s.toJson();
        check(json.contains("\"format\": \"netsketch-survey\"") && json.contains("\"utilization\": 0.42") && json.contains("Cafe \\\"Corner\\\""), "json");
        check(new Survey().toJson().contains("\"points\": []"), "empty survey");
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
