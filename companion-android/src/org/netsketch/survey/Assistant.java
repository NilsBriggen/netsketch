package org.netsketch.survey;

import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * The phone's assistant: plain-language findings (error / warning / tip) from what the phone hears now,
 * the watch log and the measured points, plus what it found out about the internet side. Plain Java, so
 * test/AssistantTest.java checks it on a computer.
 *
 * A phone sees a lot but not everything: it can't see which way an extender links back to the router,
 * so those findings are worded as likely and say why.
 */
public final class Assistant {
    private Assistant() {}

    /** What the phone found out about the internet side (null fields = not known). */
    public static final class Facts {
        public boolean checked;
        public String upnpIp, publicIp, router = "", privateDns = "";
        public Boolean ipv6;
        public Double dnsMs;
        public final List<String> dns = new ArrayList<>();
    }

    public static final class Input {
        public Collection<Air.Ap> aps = new ArrayList<>();
        public Survey.Connected connected;
        public Set<String> mine = new HashSet<>();
        public List<Survey.Event> events = new ArrayList<>();
        public List<Survey.Point> points = new ArrayList<>();
        public Facts facts;
        public long now = System.currentTimeMillis();
    }

    private static final String ERROR = "error", WARNING = "warning", TIP = "tip";
    // Not Set.of: that needs Android 11, the app runs on 10.
    private static final List<String> UPPER_5GHZ = Arrays.asList("US", "CA", "AU", "NZ", "CN", "IN", "SG", "KR", "TW", "BR", "MX");
    private static final List<Integer> GRID_24 = Arrays.asList(1, 5, 6, 9, 11, 13);

    public static List<Survey.Finding> run(Input in) {
        Map<String, Survey.Finding> out = new LinkedHashMap<>();
        Collection<Air.Ap> aps = in.aps;
        if (aps.isEmpty()) {
            // Not scanning live: the newest measured point is the picture of the air.
            for (int i = in.points.size() - 1; i >= 0 && aps.isEmpty(); i--) {
                if (in.points.get(i).scans == 0) continue;
                List<Air.Ap> l = new ArrayList<>();
                for (Survey.Seen s : in.points.get(i).networks) l.add(Air.Ap.of(s));
                aps = l;
            }
        }
        String cs = in.connected == null ? "" : in.connected.ssid, cb = in.connected == null ? "" : in.connected.bssid;
        List<Air.Box> boxes = Air.group(aps, in.mine, cs, cb);
        Set<String> names = Air.ownNames(in.mine, cs);
        List<Survey.Seen> own = new ArrayList<>(), others = new ArrayList<>();
        Set<String> ownKeys = new HashSet<>();
        for (Air.Box b : boxes) {
            if (b.own) ownKeys.add(b.key);
            for (Air.Ap a : b.radios) (b.own ? own : others).add(a.last);
        }
        if (!cb.isEmpty()) ownKeys.add(Air.boxKey(cb));

        List<Survey.Event> events = merge(in.events, null);
        channels(out, own, others);
        jumping(out, events, own, others, ownKeys, names);
        settings(out, boxes, own, others, names);
        sticky(out, aps, in, ownKeys);
        watched(out, events);
        uplinks(out, in.points, in.mine);
        network(out, in.facts);

        List<Survey.Finding> l = new ArrayList<>(out.values());
        l.sort((a, b) -> Integer.compare(rank(a.level), rank(b.level)));
        return l;
    }

    private static int rank(String level) {
        return level.equals(ERROR) ? 0 : level.equals(WARNING) ? 1 : 2;
    }

    private static void add(Map<String, Survey.Finding> out, String level, String key, String bssid, String text) {
        out.putIfAbsent(key, new Survey.Finding(level, key, bssid, text));
    }

    /** Events from two sources (the watch log, this session's scans) without duplicates, oldest first. */
    public static List<Survey.Event> merge(List<Survey.Event> a, List<Survey.Event> b) {
        Map<String, Survey.Event> m = new LinkedHashMap<>();
        for (List<Survey.Event> l : Arrays.asList(a, b)) {
            if (l == null) continue;
            for (Survey.Event e : l) m.putIfAbsent(e.at + e.kind + e.bssid.toLowerCase(Locale.ROOT), e);
        }
        List<Survey.Event> out = new ArrayList<>(m.values());
        out.sort((x, y) -> Long.compare(x.at, y.at));
        return out;
    }

    private static String clock(long at) {
        return DateTimeFormatter.ofPattern("HH:mm").withZone(ZoneId.systemDefault()).format(Instant.ofEpochMilli(at));
    }

    private static String name(Survey.Seen s) {
        return s.ssid.isEmpty() ? T.t("(hidden)") : s.ssid;
    }

    // ---------- channels ----------

    /** How much a neighbour network takes from a channel: loud and busy counts most. */
    static double weight(Survey.Seen s) {
        double strength = Math.max(0, Math.min(1, (s.rssi + 85) / 20.0));
        double busy = s.utilization == null ? 0.8 : 0.3 + s.utilization;
        return strength * busy;
    }

    static double cost(List<Survey.Seen> others, String band, int freq, int width) {
        int[] sp = Air.span(freq, width);
        double c = 0;
        for (Survey.Seen o : others) if (Survey.bandOf(o.freq).equals(band) && Air.overlap(Air.span(o.freq, o.width), sp)) c += weight(o);
        return c;
    }

    /** The channels worth suggesting on a band, for a network of this width. */
    static List<Integer> candidates(String band, int width, String country, boolean upperInUse) {
        List<Integer> c = new ArrayList<>();
        if (band.equals("2.4 GHz")) {
            c.add(1);
            c.add(6);
            c.add(11);
            // Europe and most of the world also have 13 (the 1, 5, 9, 13 plan); the Americas don't.
            if (!country.isEmpty() && !country.equals("US") && !country.equals("CA")) {
                c.add(5);
                c.add(9);
                c.add(13);
            }
            return c;
        }
        if (band.equals("6 GHz")) {
            for (int ch = 5; ch <= 85; ch += 16) c.add(ch); // shortcut: the preferred scanning channels of the lower 6 GHz part only
            return c;
        }
        boolean upper = upperInUse || UPPER_5GHZ.contains(country);
        Set<Integer> all = new LinkedHashSet<>();
        for (int ch = 36; ch <= 64; ch += 4) all.add(ch);
        for (int ch = 100; ch <= 144; ch += 4) all.add(ch);
        if (upper) for (int ch = 149; ch <= 165; ch += 4) all.add(ch);
        int w = Math.min(width, 160);
        for (int ch : all) {
            int f = Air.freqOf(ch, band);
            if (Air.span(f, w)[0] != f - 10) continue; // the lowest 20 MHz of its block
            boolean fits = true;
            for (int sub = ch; sub < ch + w / 5; sub += 4) fits &= all.contains(sub);
            if (fits) c.add(ch);
        }
        return c;
    }

    /** {best channel, its cost} for a network like s, or null. Radar channels count a little worse. */
    static double[] best(Survey.Seen s, List<Survey.Seen> others, String country, boolean upperInUse) {
        String band = Survey.bandOf(s.freq);
        int width = band.equals("2.4 GHz") ? 20 : s.width;
        double[] best = null;
        for (int ch : candidates(band, width, country, upperInUse)) {
            double c = cost(others, band, Air.freqOf(ch, band), width) + (band.equals("5 GHz") && Air.dfs(ch) ? 0.3 : 0);
            if (best == null || c < best[1] - 1e-9) best = new double[] {ch, c};
        }
        return best;
    }

    private static String country(List<Survey.Seen> own) {
        for (Survey.Seen s : own) if (!s.country.isEmpty()) return s.country;
        return "";
    }

    private static boolean upperInUse(List<Survey.Seen> own) {
        for (Survey.Seen s : own) if (s.channel >= 149 && s.freq < 5925) return true;
        return false;
    }

    private static void channels(Map<String, Survey.Finding> out, List<Survey.Seen> own, List<Survey.Seen> others) {
        String country = country(own);
        boolean upper = upperInUse(own);
        for (Survey.Seen s : own) {
            String band = Survey.bandOf(s.freq);
            double cur = cost(others, band, s.freq, s.width);
            if (cur < 0.7) continue;
            int n = 0;
            Survey.Seen loud = null;
            int[] sp = Air.span(s.freq, s.width);
            for (Survey.Seen o : others) {
                if (!Survey.bandOf(o.freq).equals(band) || !Air.overlap(Air.span(o.freq, o.width), sp) || weight(o) < 0.05) continue;
                n++;
                if (loud == null || o.rssi > loud.rssi) loud = o;
            }
            if (loud == null) continue;
            String text = T.f("Your %s network '%s' on channel %d shares the air with %d neighbouring networks (loudest: '%s', %.0f dBm).", band, name(s), s.channel, n, name(loud), loud.rssi);
            double[] b = best(s, others, country, upper);
            if (b != null && (int) b[0] != s.channel && b[1] <= cur * 0.5) text += " " + T.f("Channel %d is the quietest here.", (int) b[0]);
            add(out, cur >= 1.5 ? WARNING : TIP, "crowded:" + band + ":" + s.channel, s.bssid, text);
        }
    }

    // ---------- channel jumping ----------

    private static boolean ownEvent(Survey.Event e, Set<String> ownKeys, Set<String> names) {
        return ownKeys.contains(Air.boxKey(e.bssid)) || names.contains(e.ssid) || (!e.ssid.isEmpty() && Air.extBase(e.ssid, names) != null);
    }

    private static void jumping(Map<String, Survey.Finding> out, List<Survey.Event> events, List<Survey.Seen> own, List<Survey.Seen> others, Set<String> ownKeys, Set<String> names) {
        Map<String, List<Survey.Event>> byBox = new LinkedHashMap<>();
        List<Survey.Event> mineEv = new ArrayList<>(), theirs = new ArrayList<>();
        for (Survey.Event e : events) {
            if (!e.kind.equals("channel")) continue;
            byBox.computeIfAbsent(Air.boxKey(e.bssid) + "/" + e.note, k -> new ArrayList<>()).add(e);
            (ownEvent(e, ownKeys, names) ? mineEv : theirs).add(e);
        }
        int jumpy = 0;
        for (List<Survey.Event> l : byBox.values()) {
            Survey.Event first = l.get(0);
            boolean mine = ownEvent(first, ownKeys, names);
            if (l.size() < 2) continue;
            if (!mine) {
                jumpy++;
                continue;
            }
            StringBuilder path = new StringBuilder(first.from);
            for (Survey.Event e : l) path.append(" → ").append(e.to);
            add(out, WARNING, "jump:" + Air.boxKey(first.bssid) + ":" + first.note, first.bssid,
                    T.f("Your access point %s changed its %s channel %d times (%s). Each change cuts devices off for a moment. Automatic channel selection does this: set a fixed channel.", Air.label(first.ssid, first.bssid), first.note, l.size(), path));
        }
        if (jumpy > 0) add(out, TIP, "jumpy-neighbours", "", T.f("%d neighbouring access points change channel by themselves, too.", jumpy));
        String country = country(own);
        boolean upper = upperInUse(own);
        for (Survey.Event m : mineEv) {
            for (Survey.Event t : theirs) {
                if (!m.note.equals(t.note)) continue;
                boolean close = Math.abs(m.at - t.at) <= 10 * 60_000;
                boolean onto = m.to.equals(t.to) || m.to.equals(t.from) || t.to.equals(m.from);
                if (!close && !onto) continue;
                String fix = T.t("a fixed channel");
                for (Survey.Seen s : own) {
                    if (!Survey.bandOf(s.freq).equals(m.note)) continue;
                    double[] b = best(s, others, country, upper);
                    if (b != null) fix = T.f("channel %d", (int) b[0]);
                }
                add(out, WARNING, "pingpong:" + m.note, m.bssid, T.f("Your router and a neighbour ('%s') keep changing %s channel around each other (yours %s → %s at %s, theirs %s → %s at %s). Both pick channels automatically and chase the same quiet spot. Set yours to %s and this stops.",
                        t.ssid.isEmpty() ? T.t("(hidden)") : t.ssid, m.note, m.from, m.to, clock(m.at), t.from, t.to, clock(t.at), fix));
            }
        }
        // A neighbour moving onto the channel one of yours is on now.
        for (Survey.Event t : theirs) {
            for (Survey.Seen s : own) {
                if (!Survey.bandOf(s.freq).equals(t.note) || !t.to.equals(String.valueOf(s.channel))) continue;
                add(out, WARNING, "onto:" + t.bssid.toLowerCase(Locale.ROOT), t.bssid, T.f("'%s' moved onto your %s channel %s at %s.", t.ssid.isEmpty() ? T.t("(hidden)") : t.ssid, t.note, t.to, clock(t.at)));
            }
        }
    }

    // ---------- settings of your own access points ----------

    private static boolean guest(String ssid) {
        return ssid.toLowerCase(Locale.ROOT).matches(".*(guest|gast|invit|iot).*");
    }

    private static void settings(Map<String, Survey.Finding> out, List<Air.Box> boxes, List<Survey.Seen> own, List<Survey.Seen> others, Set<String> names) {
        Set<String> countries = new LinkedHashSet<>();
        for (Survey.Seen s : own) {
            String band = Survey.bandOf(s.freq);
            if (!s.country.isEmpty()) countries.add(s.country);
            if (band.equals("2.4 GHz") && s.channel >= 1 && s.channel <= 13 && !GRID_24.contains(s.channel)) {
                add(out, WARNING, "grid:" + s.channel, s.bssid, T.f("Channel %d sits between the non-overlapping ones (1, 6, 11 — or 1, 5, 9, 13 in Europe) and collides with neighbours on both sides.", s.channel));
            }
            if (band.equals("2.4 GHz") && s.width >= 40) {
                add(out, WARNING, "wide24:" + s.ssid, s.bssid, T.f("Your 2.4 GHz network '%s' uses 40 MHz. On 2.4 GHz that covers most of the band and collides with every neighbour: set it to 20 MHz (the speed comes from 5 GHz).", name(s)));
            }
            if (band.equals("5 GHz") && s.width >= 160) {
                int n = 0;
                for (Survey.Seen o : others) if (Survey.bandOf(o.freq).equals(band) && o.rssi >= -80 && Air.overlap(Air.span(o.freq, o.width), Air.span(s.freq, s.width))) n++;
                if (n >= 2) add(out, TIP, "wide5:" + s.ssid, s.bssid, T.f("'%s' uses 160 MHz on 5 GHz, where %d neighbouring networks overlap it. 80 MHz is often faster here: less air to share, fewer collisions.", name(s), n));
            }
            if (band.equals("5 GHz") && Air.dfs(s.channel)) {
                add(out, TIP, "dfs:" + s.channel, s.bssid, T.f("Your 5 GHz network '%s' is on channel %d, a radar channel (DFS): when the router hears radar it must leave for a while and devices drop briefly. If you see short dropouts, pick one of 36–48.", name(s), s.channel));
            }
            if (s.ssid.isEmpty()) continue;
            if (s.wps) add(out, WARNING, "wps:" + s.ssid, s.bssid, T.f("WPS is switched on for '%s'. Its PIN method can be cracked: turn WPS off in the router unless you are pairing something right now.", s.ssid));
            if (s.security.equals("WEP")) add(out, ERROR, "wep:" + s.ssid, s.bssid, T.f("'%s' still uses WEP, which is cracked in minutes. Switch it to WPA3.", s.ssid));
            else if (s.security.equals("Open")) add(out, WARNING, "open:" + s.ssid, s.bssid, T.f("'%s' is open (no password): anyone nearby can join it.", s.ssid));
            else if (s.caps.contains("TKIP")) add(out, WARNING, "tkip:" + s.ssid, s.bssid, T.f("'%s' still allows the old TKIP encryption: it's weak, and devices using it are held to 54 Mbit/s. Choose WPA2 (AES) or WPA3.", s.ssid));
            if (s.security.equals("WPA2") && (s.standard.equals("Wi-Fi 6") || s.standard.equals("Wi-Fi 7"))) {
                add(out, TIP, "wpa3:" + s.ssid, s.bssid, T.f("'%s' uses only WPA2 although it runs on %s gear, which can do WPA3. The 'WPA2/WPA3' mode keeps older devices working and protects the rest better.", s.ssid, s.standard));
            }
            if (s.utilization != null && s.utilization > 0.6) {
                add(out, WARNING, "load:" + s.bssid.toLowerCase(Locale.ROOT), s.bssid, T.f("Your access point %s reports its channel %.0f%% busy: devices wait their turn, so everything feels slower.", Air.label(s.ssid, s.bssid), s.utilization * 100));
            }
        }
        if (countries.size() >= 2) add(out, WARNING, "country", "", T.f("Your access points announce different countries (%s). Set the same country everywhere, or some devices won't use some channels.", String.join(", ", countries)));

        // Names: one per band on one box; an extender with its own name.
        Set<String> all = new LinkedHashSet<>();
        for (Air.Box b : boxes) {
            for (Air.Ap a : b.radios) if (!a.last.ssid.isEmpty()) all.add(a.last.ssid);
            if (!b.own) continue;
            Map<String, Set<String>> bandsOf = new HashMap<>();
            for (Air.Ap a : b.radios) if (!a.last.ssid.isEmpty() && !guest(a.last.ssid)) bandsOf.computeIfAbsent(a.last.ssid, k -> new HashSet<>()).add(Survey.bandOf(a.last.freq));
            String only24 = null, only5 = null;
            for (Map.Entry<String, Set<String>> e : bandsOf.entrySet()) {
                if (e.getValue().equals(Collections.singleton("2.4 GHz"))) only24 = e.getKey();
                if (e.getValue().equals(Collections.singleton("5 GHz"))) only5 = e.getKey();
            }
            if (only24 != null && only5 != null) {
                add(out, TIP, "bandnames:" + b.key, b.radios.get(0).last.bssid, T.f("'%s' (2.4 GHz) and '%s' (5 GHz) are separate names on one box. With one name for both, devices pick the faster band themselves and switch when it fades.", only24, only5));
            }
        }
        for (String ssid : all) {
            String base = Air.extBase(ssid, names);
            if (base != null) add(out, WARNING, "extname:" + ssid, "", T.f("'%s' has its own name, so devices don't roam between it and '%s': they hold on to one until it fails. Give the extender the same name and password (or use mesh).", ssid, base));
        }
        // One name from several boxes, but no fast roaming announced anywhere.
        for (String n : names) {
            Set<String> keys = new HashSet<>();
            boolean known = true, any = false;
            for (Air.Box b : boxes) {
                for (Air.Ap a : b.radios) {
                    if (!a.last.ssid.equals(n)) continue;
                    keys.add(b.key);
                    known &= a.last.ies;
                    any |= a.last.k || a.last.v || a.last.r;
                }
            }
            if (keys.size() >= 2 && known && !any) {
                add(out, TIP, "kvr:" + n, "", T.f("'%s' comes from %d access points, but none of them announces fast roaming (802.11k/v/r). Switching it on in the router (often called 'fast roaming' or 'steering') helps devices move to the nearer one.", n, keys.size()));
            }
        }
    }

    // ---------- sticking to a far access point ----------

    private static void sticky(Map<String, Survey.Finding> out, Collection<Air.Ap> aps, Input in, Set<String> ownKeys) {
        Survey.Connected c = in.connected;
        if (c != null && !c.bssid.isEmpty() && !c.ssid.isEmpty()) {
            long from = in.now - 3 * 60_000;
            Air.Ap mine = null;
            for (Air.Ap a : aps) if (a.last.bssid.equalsIgnoreCase(c.bssid)) mine = a;
            if (mine != null) {
                double[] m = mine.since(from);
                long first = Long.MAX_VALUE;
                for (int i = 0; i < mine.n; i++) if (mine.at[i] >= from) first = Math.min(first, mine.at[i]);
                if (m[1] >= 2 && mine.lastSeen() - first >= 60_000 && m[0] < -60) {
                    for (Air.Ap o : aps) {
                        if (!o.last.ssid.equals(c.ssid) || Air.boxKey(o.last.bssid).equals(Air.boxKey(c.bssid))) continue;
                        double[] x = o.since(from);
                        if (x[1] >= 2 && x[0] - m[0] >= 10) {
                            add(out, WARNING, "sticky:" + c.bssid.toLowerCase(Locale.ROOT), c.bssid, T.f("For the last few minutes the phone stayed on %s at %.0f dBm although %s offered %.0f dBm. Devices that stick like this are slow; fast roaming (802.11k/v/r), or a little less transmit power on the far one, helps.",
                                    Air.label(c.ssid, c.bssid), m[0], Air.label(o.last.ssid, o.last.bssid), x[0]));
                            break;
                        }
                    }
                }
            }
        }
        int n = 0;
        for (Survey.Point p : in.points) {
            if (p.connected == null) continue;
            double other = -200;
            for (Survey.Seen s : p.networks) {
                if (s.ssid.equals(p.connected.ssid) && !Air.boxKey(s.bssid).equals(Air.boxKey(p.connected.bssid))) other = Math.max(other, s.rssi);
            }
            if (p.connected.rssi < -65 && other - p.connected.rssi >= 10) n++;
        }
        if (n > 0) add(out, WARNING, "sticky-points", "", T.f("At %d of your spots the phone stayed on a far access point although a nearer one of yours was at least 10 dB stronger.", n));
    }

    // ---------- what the watch saw ----------

    private static void watched(Map<String, Survey.Finding> out, List<Survey.Event> events) {
        List<Survey.Event> roams = new ArrayList<>(), drops = new ArrayList<>(), spikes = new ArrayList<>(), loss = new ArrayList<>(), dips = new ArrayList<>();
        for (Survey.Event e : events) {
            switch (e.kind) {
                case "roam": roams.add(e); break;
                case "drop": drops.add(e); break;
                case "spike": spikes.add(e); break;
                case "loss": loss.add(e); break;
                case "dip": dips.add(e); break;
                default: break;
            }
        }
        // Back and forth between the same two access points, three times within ten minutes.
        for (int i = 0; i + 2 < roams.size(); i++) {
            Survey.Event a = roams.get(i), b = roams.get(i + 1), c = roams.get(i + 2);
            if (c.at - a.at > 10 * 60_000) continue;
            String pa = pair(a), pb = pair(b), pc = pair(c);
            if (!pa.equals(pb) || !pa.equals(pc)) continue;
            int n = 0;
            for (Survey.Event r : roams) if (pair(r).equals(pa)) n++;
            add(out, WARNING, "pingpong-roam", a.bssid, T.f("The phone switched back and forth between %s and %s %d times. Both reach this spot about equally well, so devices keep hopping; lower the transmit power of one, or move it a little.", a.from, a.to, n));
            break;
        }
        if (drops.size() >= 2) {
            double longest = 0;
            for (Survey.Event d : drops) longest = Math.max(longest, d.value);
            add(out, ERROR, "drops", "", T.f("Wi-Fi dropped %d times while watching (the longest for %.0f s). Where the signal is good, that points to the router restarting or overloaded.", drops.size(), longest));
        } else if (drops.size() == 1) {
            add(out, WARNING, "drops", "", T.f("Wi-Fi dropped once while watching, for %.0f s.", drops.get(0).value));
        }
        if (spikes.size() >= 4) {
            double sum = 0, sq = 0;
            int n = spikes.size() - 1;
            for (int i = 1; i < spikes.size(); i++) {
                double d = (spikes.get(i).at - spikes.get(i - 1).at) / 1000.0;
                sum += d;
                sq += d * d;
            }
            double mean = sum / n, sd = Math.sqrt(Math.max(0, sq / n - mean * mean));
            if (mean >= 5 && mean <= 3600 && sd / mean < 0.25) {
                add(out, WARNING, "periodic-spikes", "", T.f("The delay jumps about every %.0f s (%d times): something runs on a timer, often power saving or the phone or router scanning in the background.", mean, spikes.size()));
            }
        }
        if (spikes.size() >= 3) {
            double worst = 0;
            for (Survey.Event s : spikes) worst = Math.max(worst, s.value);
            add(out, WARNING, "spikes", "", T.f("The router's delay jumped above 100 ms %d times (worst %.0f ms). Video calls and games stutter then.", spikes.size(), worst));
        }
        if (loss.size() >= 2) add(out, WARNING, "loss", "", T.f("The router stopped answering in bursts %d times: the connection hiccups even when it stays up.", loss.size()));
        if (dips.size() >= 3) {
            double low = 0;
            for (Survey.Event d : dips) low = Math.min(low, d.value);
            add(out, TIP, "dips", "", T.f("The signal dropped sharply %d times (lowest %.0f dBm). People, doors or a running microwave in the way can do this.", dips.size(), low));
        }
    }

    private static String pair(Survey.Event e) {
        String a = e.from.toLowerCase(Locale.ROOT), b = e.to.toLowerCase(Locale.ROOT);
        return a.compareTo(b) < 0 ? a + "|" + b : b + "|" + a;
    }

    // ---------- extenders and mesh nodes: their link back ----------

    private static double level(Survey.Point p, String key) {
        double best = -100;
        for (Survey.Seen s : p.networks) if (Air.boxKey(s.bssid).equals(key)) best = Math.max(best, s.rssi);
        return best;
    }

    private static void uplinks(Map<String, Survey.Finding> out, List<Survey.Point> points, Set<String> mine) {
        List<Survey.Point> pts = new ArrayList<>();
        for (Survey.Point p : points) if (p.scans > 0) pts.add(p);
        Map<String, String> nodes = new LinkedHashMap<>();
        Map<String, Integer> heard = new HashMap<>();
        for (Survey.Point p : pts) {
            Set<String> here = new HashSet<>();
            for (Survey.Seen s : p.networks) {
                if (!Survey.isOwn(p, s, mine)) continue;
                String k = Air.boxKey(s.bssid);
                nodes.putIfAbsent(k, Air.label(s.ssid, s.bssid));
                here.add(k);
            }
            for (String k : here) heard.merge(k, 1, Integer::sum);
        }
        if (nodes.size() >= 2 && pts.size() >= 2) {
            // The main router: the one with the lowest router delay through it (an extender adds a hop), else the one heard most.
            Map<String, double[]> pingVia = new HashMap<>();
            for (Survey.Point p : pts) {
                if (p.connected == null || p.ping == null) continue;
                double[] a = pingVia.computeIfAbsent(Air.boxKey(p.connected.bssid), k -> new double[2]);
                a[0] += p.ping.avgMs;
                a[1]++;
            }
            String main = null;
            if (pingVia.size() >= 2) {
                double low = Double.MAX_VALUE;
                for (Map.Entry<String, double[]> e : pingVia.entrySet()) {
                    double avg = e.getValue()[0] / e.getValue()[1];
                    if (nodes.containsKey(e.getKey()) && avg < low) {
                        low = avg;
                        main = e.getKey();
                    }
                }
            }
            if (main == null) {
                int most = -1;
                for (String k : nodes.keySet()) {
                    if (heard.get(k) > most) {
                        most = heard.get(k);
                        main = k;
                    }
                }
            }
            for (String x : nodes.keySet()) {
                if (x.equals(main)) continue;
                Survey.Point peak = null;
                double top = -200;
                for (Survey.Point p : pts) {
                    double l = level(p, x);
                    if (l > top) {
                        top = l;
                        peak = p;
                    }
                }
                if (peak == null || top < -60) continue; // never measured near it
                double m = level(peak, main);
                String why = T.t("A phone can't see which way an extender really connects: this is an estimate from where its signal peaked.");
                if (m < -75) {
                    add(out, WARNING, "uplink-weak:" + x, "", T.f("%s: where its signal is strongest ('%s'), your main router (probably %s) reached only %s. Its own link back is likely weak, which slows everything connected to it. Move it a little closer to the router, or connect it with a cable.",
                            nodes.get(x), peak.name, nodes.get(main), m <= -100 ? T.t("nothing") : T.f("%.0f dBm", m)) + " " + why);
                }
                for (String y : nodes.keySet()) {
                    if (y.equals(x) || y.equals(main)) continue;
                    double l = level(peak, y);
                    if (l > -100 && l - m >= 8) {
                        add(out, TIP, "uplink-other:" + x, "", T.f("At '%s', near %s, %s was %.0f dB louder than your main router. If %s links straight to the router, it uses the weaker path; if it links through %s, data crosses the air twice. Check its uplink in the router's or mesh app's overview.",
                                peak.name, nodes.get(x), nodes.get(y), l - m, nodes.get(x), nodes.get(y)) + " " + why);
                        break;
                    }
                }
            }
        }
        // Strong signal to one node but much slower than at the others: that node's link back is the bottleneck.
        Map<String, double[]> fast = new LinkedHashMap<>();
        for (Survey.Point p : pts) {
            if (p.connected == null || p.speed == null || p.connected.rssi < -60) continue;
            double[] a = fast.computeIfAbsent(Air.boxKey(p.connected.bssid), k -> new double[2]);
            a[0] += p.speed.downMbps;
            a[1]++;
        }
        for (Map.Entry<String, double[]> e : fast.entrySet()) {
            double sum = 0, n = 0;
            for (Map.Entry<String, double[]> o : fast.entrySet()) {
                if (o == e) continue;
                sum += o.getValue()[0];
                n += o.getValue()[1];
            }
            double here = e.getValue()[0] / e.getValue()[1];
            if (n > 0 && here < 0.5 * sum / n) {
                String label = nodes.getOrDefault(e.getKey(), e.getKey());
                add(out, WARNING, "uplink-slow:" + e.getKey(), "", T.f("Connected to %s with a strong signal, the phone got only %.0f Mbit/s, against %.0f Mbit/s at your other access points. Its link back to the router is probably the bottleneck.", label, here, sum / n));
            }
        }
    }

    // ---------- the internet side ----------

    private static void network(Map<String, Survey.Finding> out, Facts f) {
        if (f == null || !f.checked) return;
        if (f.upnpIp != null) {
            if (Lan.cgnat(f.upnpIp)) {
                add(out, WARNING, "cgnat", "", T.f("Your provider shares one public address among many customers (carrier-grade NAT; your router got %s). Port forwarding, and some games or cameras, won't work from outside: ask for a public IPv4 address, or use IPv6.", f.upnpIp));
            } else if (Lan.isPrivate(f.upnpIp)) {
                add(out, WARNING, "double-nat", "", T.f("Double NAT: your router's internet side has the private address %s, so it sits behind another router (often the provider's modem). Put that one in bridge mode, or make yours the only router.", f.upnpIp));
            } else if (f.publicIp != null && Lan.isV4(f.publicIp) && !f.publicIp.equals(f.upnpIp)) {
                add(out, WARNING, "cgnat", "", T.f("Your router's internet address (%s) differs from the one the internet sees (%s): something between you and the internet translates addresses again (double NAT or carrier-grade NAT).", f.upnpIp, f.publicIp));
            }
        }
        if (!f.router.isEmpty()) add(out, TIP, "router", "", T.f("Your router introduces itself as %s.", f.router));
        if (Boolean.FALSE.equals(f.ipv6)) add(out, TIP, "no-ipv6", "", T.t("No IPv6 on this Wi-Fi. Most providers offer it, and it avoids their address sharing: switch it on in the router if you can."));
        if (f.dnsMs != null && f.dnsMs >= 150) add(out, WARNING, "dns-slow", "", T.f("Looking up a name takes %.0f ms here (good is under 50 ms). Every new website waits for it: try another DNS server in the router.", f.dnsMs));
        if (!f.dns.isEmpty()) {
            String who = String.join(", ", f.dns);
            add(out, TIP, "dns", "", f.privateDns.isEmpty() ? T.f("Names are looked up by %s.", who) : T.f("Names are looked up by %s (encrypted, Private DNS).", f.privateDns));
        }
    }
}
