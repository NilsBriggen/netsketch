import org.netsketch.survey.Air;
import org.netsketch.survey.Assistant;
import org.netsketch.survey.Survey;
import org.netsketch.survey.T;

import java.util.ArrayList;
import java.util.List;
import java.util.Set;

/** Checks the phone's assistant: each kind of finding from a small, made-up home. Run by build.sh. */
public class AssistantTest {
    static void check(boolean ok, String what) {
        if (!ok) throw new AssertionError(what);
    }

    static Survey.Seen seen(String ssid, String bssid, int freq, int width, double rssi) {
        Survey.Seen s = new Survey.Seen();
        s.ssid = ssid;
        s.bssid = bssid;
        s.freq = freq;
        s.channel = Survey.channelOf(freq);
        s.width = width;
        s.rssi = rssi;
        s.security = "WPA2/WPA3";
        s.ies = true;
        return s;
    }

    static Survey.Event ev(long at, String kind, String bssid, String ssid, String from, String to, double value, String note) {
        Survey.Event e = new Survey.Event();
        e.at = at;
        e.kind = kind;
        e.bssid = bssid;
        e.ssid = ssid;
        e.from = from;
        e.to = to;
        e.value = value;
        e.note = note;
        return e;
    }

    static Survey.Finding find(List<Survey.Finding> l, String keyStart) {
        for (Survey.Finding f : l) if (f.key.startsWith(keyStart)) return f;
        return null;
    }

    static String keys(List<Survey.Finding> l) {
        StringBuilder b = new StringBuilder();
        for (Survey.Finding f : l) b.append(f.level).append(' ').append(f.key).append(": ").append(f.text).append('\n');
        return b.toString();
    }

    static List<Air.Ap> aps(Survey.Seen... s) {
        List<Air.Ap> l = new ArrayList<>();
        for (Survey.Seen x : s) l.add(Air.Ap.of(x));
        return l;
    }

    public static void main(String[] args) {
        java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("UTC"));
        for (boolean german : new boolean[] {false, true}) {
            T.german = german; // every text also works in German (placeholders, formats)
            run(german);
        }
        T.german = false;
        System.out.println("assistant checks passed");
    }

    static void run(boolean german) {
        Set<String> mine = Set.of("HomeNet");
        // ---- the air now: a crowded 2.4 GHz channel 3 at 40 MHz, WPS, a radar channel, a neighbour box ----
        Survey.Seen own24 = seen("HomeNet", "d4:5d:64:aa:bb:c0", 2422, 40, -45);
        own24.wps = true;
        own24.country = "CH";
        Survey.Seen own5 = seen("HomeNet", "d4:5d:64:aa:bb:c4", 5500, 80, -50); // ch 100
        own5.country = "DE";
        own5.standard = "Wi-Fi 6";
        own5.security = "WPA2";
        own5.utilization = 0.75;
        Survey.Seen n1 = seen("Flat-3B", "3c:a6:2f:11:22:30", 2412, 20, -55);
        n1.utilization = 0.6;
        Survey.Seen n2 = seen("Flat-4A", "00:aa:bb:cc:dd:01", 2427, 20, -60);
        Survey.Seen n3 = seen("Cafe", "00:aa:bb:cc:dd:91", 2432, 20, -62);
        Survey.Seen n4 = seen("Old", "06:aa:bb:cc:dd:02", 2462, 20, -85);
        Survey.Seen tk = seen("HomeNet-Guest", "d6:5d:64:aa:bb:c0", 2422, 20, -46);
        tk.caps = "[WPA-PSK-TKIP][ESS]";
        tk.security = "WPA2";
        Assistant.Input in = new Assistant.Input();
        in.aps = aps(own24, own5, n1, n2, n3, n4, tk);
        in.mine = mine;
        List<Survey.Finding> f = Assistant.run(in);
        String all = keys(f);
        Survey.Finding crowded = find(f, "crowded:2.4 GHz:3");
        check(crowded != null && crowded.level.equals("warning") && (german || crowded.text.contains("Channel 11 is the quietest here")), "crowded 2.4 GHz and the best channel:\n" + all);
        for (String k : new String[] {"grid:3", "wide24:HomeNet", "dfs:100", "wps:HomeNet", "tkip:HomeNet-Guest", "wpa3:HomeNet", "load:", "country"}) check(find(f, k) != null, "missing " + k + ":\n" + all);
        check(find(f, "wps:Flat") == null && find(f, "crowded:5 GHz") == null, "neighbours' settings aren't yours; a quiet channel isn't crowded:\n" + all);
        check(f.get(0).level.equals("error") || f.get(0).level.equals("warning"), "worst first");
        // In Europe (country CH) the 1/5/9/13 plan is offered too; ch 13 is free.
        check(german || crowded.text.contains("quietest"), "a suggestion");

        // ---- names: one per band on one box, an extender with its own name, several boxes without fast roaming ----
        Assistant.Input names = new Assistant.Input();
        Survey.Seen a = seen("HomeNet", "d4:5d:64:aa:bb:c0", 2412, 20, -50), b = seen("HomeNet-5G", "d4:5d:64:aa:bb:c4", 5180, 80, -52);
        Survey.Seen node = seen("HomeNet", "a0:00:00:00:00:04", 2437, 20, -66), ext = seen("HomeNet_EXT", "b0:00:00:00:00:01", 2462, 20, -70);
        names.aps = aps(a, b, node, ext);
        names.mine = mine;
        String nk = keys(Assistant.run(names));
        check(nk.contains("bandnames:") && nk.contains("extname:HomeNet_EXT") && nk.contains("kvr:HomeNet"), "names:\n" + nk);
        a.k = true;
        check(!keys(Assistant.run(names)).contains("kvr:"), "one radio with 802.11k is enough");

        // ---- channel jumping: yours twice, a neighbour onto yours, both within minutes = ping-pong ----
        long t = 1_700_000_000_000L;
        Assistant.Input jump = new Assistant.Input();
        Survey.Seen mine6 = seen("HomeNet", "d4:5d:64:aa:bb:c0", 2437, 20, -45);
        jump.aps = aps(mine6, seen("Flat-3B", "3c:a6:2f:11:22:30", 2412, 20, -60));
        jump.mine = mine;
        jump.events.add(ev(t, "channel", "d4:5d:64:aa:bb:c0", "HomeNet", "1", "11", -45, "2.4 GHz"));
        jump.events.add(ev(t + 3 * 60_000, "channel", "3c:a6:2f:11:22:30", "Flat-3B", "6", "1", -60, "2.4 GHz"));
        jump.events.add(ev(t + 3 * 60_000, "channel", "3c:a6:2f:11:22:30", "Flat-3B", "6", "1", -60, "2.4 GHz")); // the same, from the other source
        jump.events.add(ev(t + 20 * 60_000, "channel", "d4:5d:64:aa:bb:c0", "HomeNet", "11", "6", -45, "2.4 GHz"));
        jump.events.add(ev(t + 50 * 60_000, "channel", "3c:a6:2f:11:22:30", "Flat-3B", "1", "6", -60, "2.4 GHz"));
        jump.events.add(ev(t + 55 * 60_000, "channel", "3c:a6:2f:11:22:30", "Flat-3B", "6", "11", -60, "2.4 GHz"));
        List<Survey.Finding> jf = Assistant.run(jump);
        String jk = keys(jf);
        check(find(jf, "jump:") != null && (german || find(jf, "jump:").text.contains("1 → 11 → 6")), "your channel jumps:\n" + jk);
        check(find(jf, "pingpong:2.4 GHz") != null && find(jf, "pingpong:").level.equals("warning") && find(jf, "jumpy-neighbours") != null, "ping-pong:\n" + jk);
        check(find(jf, "onto:3c:a6:2f:11:22:30") != null && (german || find(jf, "onto:").text.contains("channel 6 at")), "a neighbour moved onto your channel:\n" + jk);
        check(Assistant.merge(jump.events, null).size() == 5, "duplicate events merged");

        // ---- sticking to the far one while the near one is 15 dB louder, for minutes ----
        Air.Ap far = Air.Ap.of(seen("HomeNet", "d4:5d:64:aa:bb:c4", 5180, 80, -74));
        Air.Ap near = Air.Ap.of(seen("HomeNet", "a0:00:00:00:00:04", 5500, 80, -58));
        long now = 10_000_000;
        for (int i = 0; i < 4; i++) {
            Survey.Seen x = seen("HomeNet", "d4:5d:64:aa:bb:c4", 5180, 80, -73 - i % 2);
            x.at = now - 150_000 + i * 40_000;
            far = add(far, x);
            Survey.Seen y = seen("HomeNet", "a0:00:00:00:00:04", 5500, 80, -59);
            y.at = x.at;
            near = add(near, y);
        }
        Assistant.Input st = new Assistant.Input();
        st.now = now;
        st.aps = List.of(far, near);
        st.mine = mine;
        st.connected = new Survey.Connected();
        st.connected.ssid = "HomeNet";
        st.connected.bssid = "d4:5d:64:aa:bb:c4";
        check(find(Assistant.run(st), "sticky:d4:5d:64:aa:bb:c4") != null, "sticky client:\n" + keys(Assistant.run(st)));

        // ---- what the watch saw: drops, spikes every 60 s, roaming back and forth, dips, loss ----
        Assistant.Input wa = new Assistant.Input();
        wa.events.add(ev(t, "drop", "x", "HomeNet", "", "", 12, ""));
        wa.events.add(ev(t + 1000, "drop", "x", "HomeNet", "", "", 40, ""));
        for (int i = 0; i < 6; i++) wa.events.add(ev(t + 100_000 + i * 60_000, "spike", "x", "HomeNet", "", "", 200 + i, ""));
        for (int i = 0; i < 3; i++) wa.events.add(ev(t + 900_000 + i * 60_000, "roam", "", "HomeNet", i % 2 == 0 ? "aa" : "bb", i % 2 == 0 ? "bb" : "aa", -60, ""));
        for (int i = 0; i < 3; i++) wa.events.add(ev(t + 2_000_000 + i * 99_000, "dip", "x", "HomeNet", "", "", -80 - i, ""));
        wa.events.add(ev(t + 3_000_000, "loss", "x", "HomeNet", "", "", 40, ""));
        wa.events.add(ev(t + 3_100_000, "loss", "x", "HomeNet", "", "", 50, ""));
        List<Survey.Finding> wf = Assistant.run(wa);
        String wk = keys(wf);
        check(find(wf, "drops").level.equals("error") && (german || find(wf, "drops").text.contains("dropped 2 times") && find(wf, "drops").text.contains("longest for 40 s")), "drops:\n" + wk);
        check(find(wf, "periodic-spikes") != null && (german || find(wf, "periodic-spikes").text.contains("every 60 s")) && find(wf, "spikes") != null, "spikes on a timer:\n" + wk);
        check(find(wf, "pingpong-roam") != null && find(wf, "dips") != null && find(wf, "loss") != null, "roaming, dips, loss:\n" + wk);

        // ---- extenders: where each one peaks, how loud the main router is there ----
        // Router R (lowest delay through it), extender X in the far room, node Y between them.
        List<Survey.Point> pts = new ArrayList<>();
        pts.add(point("Living room", "r0:00:00:00:00:01", 3.0, 300, new double[] {-40, -78, -65}));
        pts.add(point("Hall", "y0:00:00:00:00:01", 9.0, 280, new double[] {-62, -66, -42}));
        pts.add(point("Far room", "x0:00:00:00:00:01", 14.0, 60, new double[] {-80, -45, -55}));
        Assistant.Input up = new Assistant.Input();
        up.points = pts;
        up.mine = mine;
        List<Survey.Finding> uf = Assistant.run(up);
        String uk = keys(uf);
        Survey.Finding weak = find(uf, "uplink-weak:" + Air.boxKey("x0:00:00:00:00:01"));
        check(weak != null && (german || (weak.text.contains("'Far room'") && weak.text.contains("-80 dBm") && weak.text.contains("can't see"))), "weak uplink:\n" + uk);
        check(find(uf, "uplink-other:" + Air.boxKey("x0:00:00:00:00:01")) != null, "another node louder there:\n" + uk);
        check(find(uf, "uplink-slow:" + Air.boxKey("x0:00:00:00:00:01")) != null && find(uf, "uplink-slow:" + Air.boxKey("r0:00:00:00:00:01")) == null, "the slow node:\n" + uk);
        check(find(uf, "uplink-weak:" + Air.boxKey("y0:00:00:00:00:01")) == null, "the middle node links back fine:\n" + uk);

        // ---- the internet side ----
        Assistant.Facts fa = new Assistant.Facts();
        fa.checked = true;
        fa.upnpIp = "100.72.3.4";
        fa.ipv6 = false;
        fa.dnsMs = 320.0;
        fa.dns.add("192.168.1.1");
        fa.router = "AVM FRITZ!Box 7590";
        Assistant.Input net = new Assistant.Input();
        net.facts = fa;
        String fk = keys(Assistant.run(net));
        check(fk.contains("warning cgnat") && fk.contains("tip no-ipv6") && fk.contains("warning dns-slow") && fk.contains("tip dns") && fk.contains("tip router"), "internet side:\n" + fk);
        fa.upnpIp = "192.168.178.20";
        check(keys(Assistant.run(net)).contains("double-nat"), "double NAT");
        fa.upnpIp = "85.1.2.3";
        fa.publicIp = "85.1.2.3";
        check(!keys(Assistant.run(net)).contains("nat"), "a public address is fine");
        fa.publicIp = "85.9.9.9";
        check(keys(Assistant.run(net)).contains("cgnat"), "a different public address: translated again");
        check(Assistant.run(new Assistant.Input()).isEmpty(), "nothing known, nothing said");
    }

    static Air.Ap add(Air.Ap a, Survey.Seen s) {
        Air.Tracker tr = new Air.Tracker();
        tr.aps.put(s.bssid, a);
        tr.update(List.of(s));
        return a;
    }

    /** A point connected to `via`, with speed and router delay, hearing router r0, extender x0 and node y0. */
    static Survey.Point point(String name, String via, double pingMs, double down, double[] levels) {
        Survey.Point p = new Survey.Point();
        p.name = name;
        p.scans = 3;
        String[] bss = {"r0:00:00:00:00:01", "x0:00:00:00:00:01", "y0:00:00:00:00:01"};
        for (int i = 0; i < 3; i++) p.networks.add(seen("HomeNet", bss[i], 5180 + 20 * i, 20, levels[i]));
        p.connected = new Survey.Connected();
        p.connected.ssid = "HomeNet";
        p.connected.bssid = via;
        for (Survey.Seen s : p.networks) if (s.bssid.equals(via)) p.connected.rssi = s.rssi;
        p.ping = new Survey.Ping();
        p.ping.avgMs = pingMs;
        p.speed = new Survey.Speed();
        p.speed.downMbps = down;
        return p;
    }
}
