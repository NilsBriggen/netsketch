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
            Survey.Seen h5 = seen("HomeNet", "d4:5d:64:aa:bb:c4", 5180, 80, own[i][1], "WPA2/WPA3", "Wi-Fi 6");
            // What the scans announce, and the spread over the rounds.
            h5.rssiMin = own[i][1] - 3;
            h5.rssiMax = own[i][1] + 2;
            h5.k = h5.v = h5.r = h5.ftm = true;
            h5.streams = 2;
            h5.country = "CH";
            if (i == 1) h5.distanceM = 3.4;
            p.networks.add(h5);
            Survey.Seen a = seen("Flat-3B", "3c:a6:2f:11:22:30", 2437, 20, nb[i][0], "WPA2", "Wi-Fi 6");
            a.stations = 5;
            a.utilization = 0.42;
            a.wps = true;
            a.streams = 4;
            a.country = "CH";
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
                c.txMbps = 780;
                c.rxMbps = 650;
                c.maxMbps = 1201;
                c.standard = "Wi-Fi 6";
                p.connected = c;
                p.ping = Survey.Ping.of(java.util.List.of(3.0, 4.0, 2.0, 9.0), 5);
                Survey.Speed sp = new Survey.Speed();
                sp.downMbps = 312.5;
                sp.latencyMs = 14.2;
                sp.upMbps = 48.5;
                sp.jitterMs = 2.1;
                sp.lanDownMbps = 420;
                sp.lanUpMbps = 310;
                p.speed = sp;
            }
            s.points.add(p);
        }
        // Beside the points: an hour of watching, what the assistant found, the devices on the network.
        Survey.WatchSummary w = new Survey.WatchSummary();
        w.start = java.time.Instant.parse("2026-11-04T09:00:00Z").toEpochMilli();
        w.end = java.time.Instant.parse("2026-11-04T10:00:00Z").toEpochMilli();
        w.samples = 3600;
        String[][] evs = {
            {"09:05:10", "channel", "d4:5d:64:aa:bb:c4", "HomeNet", "36", "44", "-48", "5 GHz"},
            {"09:12:00", "roam", "d4:5d:64:aa:bb:c0", "HomeNet", "d4:5d:64:aa:bb:c4", "d4:5d:64:aa:bb:c0", "-52", ""},
            {"09:20:30", "drop", "d4:5d:64:aa:bb:c0", "HomeNet", "", "", "12", ""},
            {"09:31:00", "spike", "d4:5d:64:aa:bb:c4", "HomeNet", "", "", "340", ""},
            {"09:40:00", "loss", "d4:5d:64:aa:bb:c4", "HomeNet", "", "", "40", ""},
            {"09:50:00", "dip", "d4:5d:64:aa:bb:c4", "HomeNet", "", "", "-83", ""},
        };
        for (String[] e : evs) {
            Survey.Event x = new Survey.Event();
            x.at = java.time.Instant.parse("2026-11-04T" + e[0] + "Z").toEpochMilli();
            x.kind = e[1];
            x.bssid = e[2];
            x.ssid = e[3];
            x.from = e[4];
            x.to = e[5];
            x.value = Double.parseDouble(e[6]);
            x.note = e[7];
            w.events.add(x);
        }
        s.watch = w;
        java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("UTC")); // the sample's times read the same everywhere
        org.netsketch.survey.Assistant.Input ain = new org.netsketch.survey.Assistant.Input();
        ain.points = s.points;
        ain.mine = Set.of("HomeNet");
        ain.events = w.events;
        s.findings.addAll(org.netsketch.survey.Assistant.run(ain));
        check(!s.findings.isEmpty(), "the sample has findings");
        Survey.LanDevice router = new Survey.LanDevice();
        router.ip = "192.168.1.1";
        router.name = "FRITZ!Box 7590";
        router.kind = "Router";
        router.vendor = "AVM";
        router.model = "FRITZ!Box 7590";
        Survey.LanDevice printer = new Survey.LanDevice();
        printer.ip = "192.168.1.40";
        printer.name = "Office printer";
        printer.kind = "Printer";
        s.lan.add(router);
        s.lan.add(printer);
        // A point that heard nothing isn't kept.
        check(!s.add(new Survey.Point(), null) && s.points.size() == 3, "empty point refused");
        String json = s.toJson();
        // Measuring a spot again replaces that point where it was; the new one isn't sent yet.
        Survey re = new Survey();
        Survey.Point a1 = new Survey.Point(), a2 = new Survey.Point(), again = new Survey.Point();
        for (Survey.Point x : new Survey.Point[] {a1, a2, again}) x.networks.add(seen("HomeNet", "d4:5d:64:aa:bb:c0", 2412, 20, -50, "WPA2", ""));
        a1.sent = true;
        re.add(a1, null);
        re.add(a2, null);
        check(re.add(again, a1) && re.points.size() == 2 && re.points.get(0) == again && !re.points.get(0).sent, "measured again replaces in place");
        Survey.Point late = new Survey.Point();
        late.networks.add(seen("X", "x", 2412, 20, -60, "", ""));
        check(re.add(late, a1) && re.points.size() == 3 && re.points.get(2) == late, "a replaced point that's gone: added at the end");
        check(!re.add(new Survey.Point(), again) && re.points.get(0) == again, "an empty re-measurement keeps the old point");
        check(json.contains("\"format\": \"netsketch-survey\"") && json.contains("\"utilization\": 0.42") && json.contains("Cafe \\\"Corner\\\""), "json");
        check(new Survey().toJson().contains("\"points\": []"), "empty survey");
        check(json.contains("\"up_mbps\": 48.50, \"jitter_ms\": 2.10"), "upload and jitter written");
        check(json.contains("\"lan_down_mbps\": 420.00, \"lan_up_mbps\": 310.00"), "speed to NetSketch written");
        // The new fields, each written when known.
        for (String f : new String[] {"\"rssi_min\": -61.00, \"rssi_max\": -56.00", "\"k\": true", "\"v\": true", "\"r\": true", "\"ftm\": true", "\"wps\": true", "\"streams\": 2", "\"country\": \"CH\"", "\"distance_m\": 3.40",
                "\"tx_mbps\": 780.00, \"rx_mbps\": 650.00, \"max_mbps\": 1201.00, \"standard\": \"Wi-Fi 6\"", "\"ping\": {\"avg_ms\": 4.50, \"jitter_ms\": 3.33, \"loss_pct\": 20.00, \"max_ms\": 9.00}",
                "\"watch\": {\"start\": \"2026-11-04T09:00:00Z\", \"end\": \"2026-11-04T10:00:00Z\", \"samples\": 3600", "\"kind\": \"channel\", \"bssid\": \"d4:5d:64:aa:bb:c4\", \"ssid\": \"HomeNet\", \"from\": \"36\", \"to\": \"44\"",
                "\"kind\": \"drop\"", "\"kind\": \"spike\"", "\"kind\": \"loss\"", "\"kind\": \"dip\"", "\"kind\": \"roam\"", "\"findings\": [", "\"level\": \"", "\"key\": \"",
                "\"lan\": [", "{\"ip\": \"192.168.1.1\", \"name\": \"FRITZ!Box 7590\", \"kind\": \"Router\", \"vendor\": \"AVM\", \"model\": \"FRITZ!Box 7590\"}"}) {
            check(json.contains(f), "written: " + f + "\n" + json);
        }
        check(!json.contains("\"k\": false") && !json.contains("\"streams\": 0") && !json.contains("\"country\": \"\""), "unknown or false fields are left out");
        check(Survey.Ping.of(java.util.List.of(), 5) == null, "no answers, no ping");
        // A send with only the watch, findings and devices; none at all sends nothing.
        String extras = s.toJsonExtras();
        check(extras.contains("\"points\": []") && extras.contains("\"watch\"") && extras.contains("\"lan\"") && new Survey().toJsonExtras() == null, "extras without points");
        check(!s.toJson(s.points.get(0)).contains("\"watch\""), "a single point goes alone");

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
        check(Survey.advice(s.points.get(1), mine).size() == 1 && Survey.advice(s.points.get(1), mine).get(0).startsWith("Good spot"), "good spot: " + Survey.advice(s.points.get(1), mine));
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
