import org.netsketch.survey.Air;
import org.netsketch.survey.Lan;
import org.netsketch.survey.Survey;
import org.netsketch.survey.Watch;

import java.io.File;
import java.nio.file.Files;
import java.util.List;
import java.util.Set;

/** Checks reading the scans (Air), the watch log's detection and file (Watch), and the home-network parsing (Lan). Run by build.sh. */
public class AirTest {
    static void check(boolean ok, String what) {
        if (!ok) throw new AssertionError(what);
    }

    static byte[] b(int... v) {
        byte[] a = new byte[v.length];
        for (int i = 0; i < v.length; i++) a[i] = (byte) v[i];
        return a;
    }

    static Survey.Seen seen(String ssid, String bssid, int freq, int width, double rssi) {
        Survey.Seen s = new Survey.Seen();
        s.ssid = ssid;
        s.bssid = bssid;
        s.freq = freq;
        s.channel = Survey.channelOf(freq);
        s.width = width;
        s.rssi = rssi;
        return s;
    }

    static Watch.Sample sample(long t, int rssi, String bssid, double ping) {
        Watch.Sample s = new Watch.Sample();
        s.at = t;
        s.rssi = rssi;
        s.bssid = bssid;
        s.ssid = bssid.isEmpty() ? "" : "Home";
        s.ping = ping;
        return s;
    }

    public static void main(String[] args) throws Exception {
        // ---- information elements ----
        Survey.Seen s = new Survey.Seen();
        Air.read(s, 7, -1, b('C', 'H', ' ', 1, 13, 20));
        Air.read(s, 45, -1, b(0xef, 0x09, 0x1b, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)); // 2 streams
        check(s.country.equals("CH") && s.streams == 2 && s.ies, "country and HT streams: " + s.country + " " + s.streams);
        Air.read(s, 191, -1, b(0x92, 0x01, 0x80, 0x33, 0xea, 0xff, 0, 0, 0xea, 0xff, 0, 0)); // map 0xffea: streams 1-3
        check(s.streams == 3, "VHT streams " + s.streams);
        byte[] he = new byte[21];
        he[17] = (byte) 0xaa; // streams 1-4 supported (2 = MCS 0-11)
        he[18] = (byte) 0xff;
        Air.read(s, 255, 35, he);
        check(s.streams == 4, "HE streams " + s.streams);
        Air.read(s, 255, 36, new byte[21]); // another extension: ignored
        check(!s.k && !s.v && !s.r && !s.wps, "nothing announced yet");
        Air.read(s, 54, -1, b(0x12, 0x34, 0x01));
        Air.read(s, 70, -1, b(0x73, 0, 0, 0, 0));
        Air.read(s, 127, -1, b(0x04, 0, 0x08, 0));
        Air.read(s, 221, -1, b(0x00, 0x50, 0xf2, 0x04, 0x10, 0x4a));
        check(s.r && s.k && s.v && s.wps, "k/v/r and WPS");
        Survey.Seen t = new Survey.Seen();
        Air.read(t, 70, -1, b(0x01, 0, 0, 0, 0)); // link measurement only, no neighbour reports
        Air.read(t, 127, -1, b(0x04, 0, 0x00));
        Air.read(t, 221, -1, b(0x00, 0x50, 0xf2, 0x02, 0x01)); // WMM, not WPS
        Air.read(t, 11, -1, b(3, 0, 128, 0, 0));
        check(!t.k && !t.v && !t.wps && t.stations == 3 && Math.abs(t.utilization - 128 / 255.0) < 1e-9, "not k, v, WPS; BSS load");

        // ---- channels ----
        int[] sp = Air.span(5220, 80);
        check(sp[0] == 5170 && sp[1] == 5250, "5 GHz 80 MHz block: " + sp[0] + "-" + sp[1]);
        check(Air.span(2437, 20)[0] == 2427 && Air.overlap(Air.span(2412, 20), Air.span(2427, 20)) && !Air.overlap(Air.span(2412, 20), Air.span(2432, 20)), "2.4 GHz overlap: 1/5 don't, 1/4 do");
        check(Air.span(6115, 160)[0] == 5945 + 160, "6 GHz block");
        check(Air.freqOf(36, "5 GHz") == 5180 && Air.freqOf(13, "2.4 GHz") == 2472 && Air.dfs(100) && !Air.dfs(36), "frequencies, DFS");

        // ---- boxes ----
        check(Air.boxKey("d4:5d:64:aa:bb:c0").equals(Air.boxKey("D6:5D:64:AA:BB:C7")), "locally administered twin of one box");
        check(!Air.boxKey("d4:5d:64:aa:bb:c0").equals(Air.boxKey("d4:5d:64:aa:bc:c0")), "another box");
        check("HomeNet".equals(Air.extBase("HomeNet_EXT", Set.of("HomeNet"))) && Air.extBase("HomeNet-5G", Set.of("HomeNet")) == null, "extender names");
        List<Air.Ap> aps = List.of(Air.Ap.of(seen("HomeNet", "d4:5d:64:aa:bb:c0", 2412, 20, -50)), Air.Ap.of(seen("", "d6:5d:64:aa:bb:c3", 5180, 80, -55)),
                Air.Ap.of(seen("Flat-3B", "3c:a6:2f:11:22:30", 2437, 20, -40)), Air.Ap.of(seen("HomeNet_EXT", "00:11:22:33:44:55", 2462, 20, -70)));
        List<Air.Box> boxes = Air.group(aps, Set.of("HomeNet"), "", "");
        check(boxes.size() == 3 && boxes.get(0).own && boxes.get(0).radios.size() == 2 && boxes.get(1).own && !boxes.get(2).own, "grouped, yours first (with the extender)");
        check(boxes.get(0).bands().equals(Set.of("2.4 GHz", "5 GHz")) && boxes.get(0).names().equals(Set.of("HomeNet")), "a box's bands and names");

        // ---- the tracker ----
        Air.Tracker tr = new Air.Tracker();
        Survey.Seen a1 = seen("Flat-3B", "3c:a6:2f:11:22:30", 2437, 20, -60);
        a1.tsUs = 1000;
        a1.at = 10_000;
        check(tr.update(List.of(a1)).isEmpty(), "first sighting");
        Survey.Seen again = seen("Flat-3B", "3c:a6:2f:11:22:30", 2462, 20, -61);
        again.tsUs = 1000; // the same cached result once more
        again.at = 10_000;
        check(tr.update(List.of(again)).isEmpty() && tr.aps.get("3c:a6:2f:11:22:30").n == 1, "the same reading isn't counted twice");
        Survey.Seen a2 = seen("Flat-3B", "3C:A6:2F:11:22:30", 2462, 20, -58);
        a2.tsUs = 2000;
        a2.at = 40_000;
        List<Survey.Event> ch = tr.update(List.of(a2));
        check(ch.size() == 1 && ch.get(0).kind.equals("channel") && ch.get(0).from.equals("6") && ch.get(0).to.equals("11") && ch.get(0).note.equals("2.4 GHz"), "channel change");
        tr.distance("3c:a6:2f:11:22:30", 4.2);
        check(tr.aps.get("3c:a6:2f:11:22:30").last.distanceM == 4.2 && tr.since(30_000).size() == 1 && tr.since(50_000).isEmpty(), "distance and recent");
        Air.Ap h = tr.aps.get("3c:a6:2f:11:22:30");
        check(h.history().length == 2 && h.since(0)[1] == 2 && Math.abs(h.since(0)[0] + 59) < 1e-6, "history");
        for (int i = 0; i < 60; i++) {
            Survey.Seen x = seen("Flat-3B", "3c:a6:2f:11:22:30", 2462, 20, -50 - i % 5);
            x.tsUs = 3000 + i;
            x.at = 50_000 + i;
            tr.update(List.of(x));
        }
        check(h.n == Air.HISTORY, "history is capped");

        // ---- the watch ----
        Watch w = new Watch();
        long t0 = 1_000_000;
        List<Survey.Event> evs = new java.util.ArrayList<>();
        for (int i = 0; i < 20; i++) evs.addAll(w.add(sample(t0 + i * 1000, -55, "aa:aa:aa:aa:aa:01", 3)));
        check(evs.isEmpty(), "a steady start: " + evs.size());
        evs.addAll(w.add(sample(t0 + 20_000, -84, "aa:aa:aa:aa:aa:01", 3))); // deep dip
        evs.addAll(w.add(sample(t0 + 21_000, -55, "aa:aa:aa:aa:aa:01", 420))); // spike
        evs.addAll(w.add(sample(t0 + 22_000, -60, "aa:aa:aa:aa:aa:02", 3))); // roam
        for (int i = 0; i < 5; i++) evs.addAll(w.add(sample(t0 + 23_000 + i * 1000, 0, "", Watch.NOT_TRIED))); // offline 5 s
        evs.addAll(w.add(sample(t0 + 28_000, -58, "aa:aa:aa:aa:aa:02", 3)));
        for (int i = 0; i < 5; i++) evs.addAll(w.add(sample(t0 + 29_000 + i * 1000, -58, "aa:aa:aa:aa:aa:02", Watch.LOST))); // the 4th lost of the last 10 is a burst
        StringBuilder kinds = new StringBuilder();
        for (Survey.Event e : evs) kinds.append(e.kind).append(' ');
        check(kinds.toString().equals("dip spike roam drop loss "), "events: " + kinds);
        Survey.Event drop = evs.get(3);
        check(drop.value == 5 && drop.at == t0 + 23_000 && drop.bssid.equals("aa:aa:aa:aa:aa:02"), "drop length " + drop.value);
        check(evs.get(2).from.equals("aa:aa:aa:aa:aa:01") && evs.get(2).to.equals("aa:aa:aa:aa:aa:02") && evs.get(1).value == 420 && evs.get(0).value == -84 && evs.get(4).value == 40, "event details");
        Survey.WatchSummary sum = w.summary();
        check(sum.samples == w.size() && sum.start == t0 && sum.events.size() == 5, "summary");
        // A watch that ends offline counts the drop until then.
        Watch w2 = new Watch();
        w2.add(sample(0, -50, "b", 2));
        w2.add(sample(1000, 0, "", Watch.NOT_TRIED));
        w2.finish(9000);
        check(w2.events().size() == 1 && w2.events().get(0).value == 8, "drop at the end");
        // A router that never answers isn't a "loss".
        Watch w3 = new Watch();
        for (int i = 0; i < 30; i++) w3.add(sample(i * 1000, -50, "b", Watch.LOST));
        check(w3.events().isEmpty(), "no loss without any answers");
        // The file: lines round-trip; older than a day is dropped on open.
        Watch.Sample ls = sample(123, -61, "aa:bb", 12.5);
        ls.ssid = "Tab\tname";
        Watch.Sample back = (Watch.Sample) Watch.parse(Watch.line(ls));
        check(back.at == 123 && back.rssi == -61 && back.ssid.equals("Tab name") && back.ping == 12.5, "sample line");
        Survey.Event eb = (Survey.Event) Watch.parse(Watch.line(evs.get(2)));
        check(eb.kind.equals("roam") && eb.to.equals("aa:aa:aa:aa:aa:02") && Watch.parse("junk") == null && Watch.parse("S\t1\tx") == null, "event line, junk");
        File f = File.createTempFile("watch", ".log");
        long now = 100 * Watch.KEEP_MS;
        Files.writeString(f.toPath(), Watch.line(sample(now - Watch.KEEP_MS - 1, -50, "old", 1)) + "\n" + Watch.line(sample(now - 1000, -50, "new", 1)) + "\nbroken\n");
        Watch w4 = new Watch();
        w4.open(f, now);
        w4.add(sample(now, -52, "new", 2));
        w4.flush();
        List<String> lines = Files.readAllLines(f.toPath());
        check(w4.size() == 2 && lines.size() == 2 && lines.get(0).contains("\tnew\t"), "kept a day: " + lines);
        w4.clear();
        check(w4.size() == 0 && Files.readAllLines(f.toPath()).isEmpty(), "cleared");
        f.delete();

        // ---- the home network ----
        Lan.Answer an = Lan.parseAnswer("HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nLocation: http://192.168.1.1:49000/igddesc.xml\r\nST: " + Lan.IGD + "\r\nSERVER: FRITZ!Box UPnP/1.0\r\n\r\n", "192.168.1.1");
        check(an != null && an.location.equals("http://192.168.1.1:49000/igddesc.xml") && an.st.equals(Lan.IGD) && an.server.startsWith("FRITZ"), "SSDP answer");
        check(Lan.parseAnswer("NOTIFY * HTTP/1.1\r\n", "x") == null && Lan.msearch("ssdp:all").contains("ST: ssdp:all\r\n\r\n"), "SSDP other");
        String desc = "<?xml version=\"1.0\"?><root xmlns=\"urn:schemas-upnp-org:device-1-0\"><device><deviceType>urn:schemas-upnp-org:device:InternetGatewayDevice:1</deviceType>"
                + "<friendlyName>FRITZ!Box 7590</friendlyName><manufacturer>AVM Berlin</manufacturer><modelName>FRITZ!Box 7590 &amp; more</modelName>"
                + "<serviceList><service><serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType><controlURL>/l3</controlURL></service></serviceList>"
                + "<deviceList><device><serviceList><service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/igdupnp/control/WANIPConn1</controlURL></service></serviceList></device></deviceList></device></root>";
        check(Lan.tag(desc, "friendlyName").equals("FRITZ!Box 7590") && Lan.tag(desc, "modelName").equals("FRITZ!Box 7590 & more") && Lan.tag(desc, "nothing").isEmpty(), "description tags");
        String[] wan = Lan.wanControl(desc, an.location);
        check(wan != null && wan[0].equals("http://192.168.1.1:49000/igdupnp/control/WANIPConn1") && wan[1].endsWith("WANIPConnection:1"), "WAN control URL");
        check(Lan.soapExternalIp(wan[1]).contains("<u:GetExternalIPAddress xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\">"), "SOAP body");
        String soap = "<s:Envelope><s:Body><u:GetExternalIPAddressResponse><NewExternalIPAddress>100.70.1.2</NewExternalIPAddress></u:GetExternalIPAddressResponse></s:Body></s:Envelope>";
        check(Lan.tag(soap, "NewExternalIPAddress").equals("100.70.1.2") && Lan.cgnat("100.70.1.2") && !Lan.cgnat("100.128.0.1") && !Lan.cgnat("85.1.2.3"), "CGNAT");
        check(Lan.isPrivate("192.168.0.1") && Lan.isPrivate("172.20.1.1") && Lan.isPrivate("10.1.1.1") && !Lan.isPrivate("172.32.0.1") && !Lan.isPrivate("2001:db8::1"), "private addresses");
        check("85.1.2.3".equals(Lan.traceIp("fl=1\nh=speed.cloudflare.com\nip=85.1.2.3\nts=1\n")) && Lan.traceIp("x") == null, "trace");
        check(Lan.kindOfService("_googlecast._tcp.").startsWith("TV") && Lan.kindOfService("_ipp._tcp").equals("Printer") && Lan.kindOfUpnp("urn:schemas-upnp-org:device:MediaRenderer:1").startsWith("TV"), "kinds");
        check(Lan.kindOfPorts(Set.of(62078, 80)).equals("iPhone or iPad") && Lan.kindOfPorts(Set.of(80)).startsWith("Device with") && Lan.kindOfPorts(Set.of()).isEmpty(), "kinds from ports");
        check(Lan.shortName("Galaxy-S23.fritz.box").equals("Galaxy-S23") && Lan.shortName("192.168.1.5").isEmpty() && Lan.shortName("3dprinter.lan").equals("3dprinter"), "host names");
        check(Lan.resolve("http://10.0.0.1:5000/desc.xml", "/ctl").equals("http://10.0.0.1:5000/ctl"), "resolve");
        System.out.println("air, watch and network checks passed");
    }
}
