package org.netsketch.survey;

import java.io.BufferedWriter;
import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.List;
import java.util.Locale;

/**
 * "Watch my Wi-Fi": a reading every second (signal, access point, link rates, the router's delay) and
 * what happened in between: roaming, drops, delay spikes, lost bursts, deep signal dips, and channel
 * changes from the scans. The last 24 hours are kept in a small text file. Plain Java, so
 * test/WatchTest.java checks the detection on a computer.
 */
public final class Watch {
    public static final long KEEP_MS = 24 * 3600_000L;
    /** A ping that got no answer, and one not tried (offline). */
    public static final double LOST = -1, NOT_TRIED = -2;

    public static final class Sample {
        public long at;
        public int rssi;
        public String bssid = "", ssid = "";
        public int tx, rx;
        public double ping = NOT_TRIED;

        public boolean online() {
            return !bssid.isEmpty();
        }
    }

    private static final Watch ONE = new Watch();

    public static Watch get() {
        return ONE;
    }

    private final List<Sample> samples = new ArrayList<>();
    private final List<Survey.Event> events = new ArrayList<>();
    /** Set by the watch service while it runs. */
    public volatile boolean active;
    public volatile long startedAt;
    private File file;
    private BufferedWriter out;
    private int unflushed;

    // What the detection remembers.
    private long offSince, lastSpike, lastLoss, lastDip;
    private String lastBssid = "", lastSsid = "";
    private final ArrayDeque<Double> answers = new ArrayDeque<>();
    private final ArrayDeque<Boolean> tries = new ArrayDeque<>();
    private final ArrayDeque<Integer> levels = new ArrayDeque<>();

    /** Reads the log (the last 24 hours of it), rewrites it without the older lines, and appends from then on. */
    public synchronized void open(File f, long now) {
        if (file != null) return;
        file = f;
        List<String> keep = new ArrayList<>();
        try {
            if (f.exists()) {
                for (String l : Files.readAllLines(f.toPath(), StandardCharsets.UTF_8)) {
                    Object o = parse(l);
                    if (o instanceof Sample && now - ((Sample) o).at <= KEEP_MS) samples.add((Sample) o);
                    else if (o instanceof Survey.Event && now - ((Survey.Event) o).at <= KEEP_MS) events.add((Survey.Event) o);
                    else continue;
                    keep.add(l);
                }
                File tmp = new File(f.getPath() + ".new");
                Files.write(tmp.toPath(), keep, StandardCharsets.UTF_8);
                Files.move(tmp.toPath(), f.toPath(), StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (IOException | RuntimeException e) {
            samples.clear();
            events.clear();
        }
        try {
            out = new BufferedWriter(new FileWriter(f, true));
        } catch (IOException e) {
            out = null; // then it only lives in memory
        }
    }

    /** A new reading: notices what changed, keeps it, and returns the events it caused. */
    public synchronized List<Survey.Event> add(Sample s) {
        List<Survey.Event> found = new ArrayList<>();
        if (!s.online()) {
            if (offSince == 0) offSince = s.at;
        } else {
            if (offSince > 0) {
                long secs = (s.at - offSince) / 1000;
                if (secs >= 2) found.add(ev(offSince, "drop", lastBssid, lastSsid, "", "", secs));
                offSince = 0;
            }
            if (!lastBssid.isEmpty() && !lastBssid.equalsIgnoreCase(s.bssid)) found.add(ev(s.at, "roam", s.bssid, s.ssid, lastBssid, s.bssid, s.rssi));
            lastBssid = s.bssid;
            lastSsid = s.ssid;
            // A deep dip: well below the last half minute's usual level, and weak.
            if (levels.size() >= 10 && s.rssi < median(levels) - 12 && s.rssi < -70 && s.at - lastDip >= 30_000) {
                found.add(ev(s.at, "dip", s.bssid, s.ssid, "", "", s.rssi));
                lastDip = s.at;
            }
            push(levels, s.rssi, 30);
            // A delay spike: far above the usual answer time.
            if (s.ping >= 0) {
                if (answers.size() >= 10 && s.ping > Math.max(100, 4 * median(answers)) && s.at - lastSpike >= 5000) {
                    found.add(ev(s.at, "spike", s.bssid, s.ssid, "", "", Math.round(s.ping)));
                    lastSpike = s.at;
                }
                push(answers, s.ping, 60);
            }
            // A burst of lost answers (only once the router is known to answer at all).
            if (s.ping != NOT_TRIED) {
                push(tries, s.ping >= 0, 10);
                int lost = 0;
                for (boolean ok : tries) if (!ok) lost++;
                if (tries.size() == 10 && lost >= 4 && answers.size() >= 10 && s.at - lastLoss >= 30_000) {
                    found.add(ev(s.at, "loss", s.bssid, s.ssid, "", "", lost * 10));
                    lastLoss = s.at;
                }
            }
        }
        samples.add(s);
        write(line(s));
        for (Survey.Event e : found) event(e);
        if (++unflushed >= 10) flush();
        return found;
    }

    /** Ends a watch: still offline then counts as a drop until now. */
    public synchronized void finish(long now) {
        if (offSince > 0 && (now - offSince) / 1000 >= 2) event(ev(offSince, "drop", lastBssid, lastSsid, "", "", (now - offSince) / 1000));
        offSince = 0;
        flush();
    }

    /** An event from outside (a channel change seen in a scan). */
    public synchronized void event(Survey.Event e) {
        events.add(e);
        write(line(e));
    }

    public synchronized void flush() {
        unflushed = 0;
        try {
            if (out != null) out.flush();
        } catch (IOException ignored) {
            // the next flush tries again
        }
    }

    /** Drops what is older than a day from memory (the file follows at the next start). */
    public synchronized void prune(long now) {
        samples.removeIf(x -> now - x.at > KEEP_MS);
        events.removeIf(x -> now - x.at > KEEP_MS);
    }

    public synchronized void clear() {
        samples.clear();
        events.clear();
        offSince = lastSpike = lastLoss = lastDip = 0;
        lastBssid = lastSsid = "";
        answers.clear();
        tries.clear();
        levels.clear();
        try {
            if (out != null) out.close();
            if (file != null) out = new BufferedWriter(new FileWriter(file, false));
        } catch (IOException e) {
            out = null;
        }
    }

    public synchronized List<Sample> samples(long since) {
        List<Sample> l = new ArrayList<>();
        for (Sample s : samples) if (s.at >= since) l.add(s);
        return l;
    }

    public synchronized List<Survey.Event> events() {
        return new ArrayList<>(events);
    }

    public synchronized int size() {
        return samples.size();
    }

    /** What goes into the file for NetSketch; null before the first reading. */
    public synchronized Survey.WatchSummary summary() {
        if (samples.isEmpty()) return null;
        Survey.WatchSummary w = new Survey.WatchSummary();
        w.start = samples.get(0).at;
        w.end = samples.get(samples.size() - 1).at;
        w.samples = samples.size();
        w.events.addAll(events);
        Collections.sort(w.events, (a, b) -> Long.compare(a.at, b.at));
        return w;
    }

    static Survey.Event ev(long at, String kind, String bssid, String ssid, String from, String to, double value) {
        Survey.Event e = new Survey.Event();
        e.at = at;
        e.kind = kind;
        e.bssid = bssid;
        e.ssid = ssid;
        e.from = from;
        e.to = to;
        e.value = value;
        return e;
    }

    private void write(String l) {
        try {
            if (out != null) {
                out.write(l);
                out.write('\n');
            }
        } catch (IOException ignored) {
            // a full disk: the reading still counts in memory
        }
    }

    private static <T> void push(ArrayDeque<T> q, T v, int max) {
        q.addLast(v);
        while (q.size() > max) q.removeFirst();
    }

    static double median(Collection<? extends Number> c) {
        double[] v = new double[c.size()];
        int i = 0;
        for (Number x : c) v[i++] = x.doubleValue();
        java.util.Arrays.sort(v);
        return v.length == 0 ? 0 : v.length % 2 == 1 ? v[v.length / 2] : (v[v.length / 2 - 1] + v[v.length / 2]) / 2;
    }

    // ---------- the file: one tab-separated line per reading or event ----------

    private static String clean(String s) {
        return s == null ? "" : s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }

    public static String line(Sample s) {
        return String.format(Locale.ROOT, "S\t%d\t%d\t%s\t%s\t%d\t%d\t%.1f", s.at, s.rssi, clean(s.bssid), clean(s.ssid), s.tx, s.rx, s.ping);
    }

    public static String line(Survey.Event e) {
        return String.format(Locale.ROOT, "E\t%d\t%s\t%s\t%s\t%s\t%s\t%.1f\t%s", e.at, clean(e.kind), clean(e.bssid), clean(e.ssid), clean(e.from), clean(e.to), e.value, clean(e.note));
    }

    /** A Sample, an Event, or null for a line it can't read. */
    public static Object parse(String l) {
        String[] p = l.split("\t", -1);
        try {
            if (p[0].equals("S") && p.length == 8) {
                Sample s = new Sample();
                s.at = Long.parseLong(p[1]);
                s.rssi = Integer.parseInt(p[2]);
                s.bssid = p[3];
                s.ssid = p[4];
                s.tx = Integer.parseInt(p[5]);
                s.rx = Integer.parseInt(p[6]);
                s.ping = Double.parseDouble(p[7]);
                return s;
            }
            if (p[0].equals("E") && p.length == 9) {
                Survey.Event e = ev(Long.parseLong(p[1]), p[2], p[3], p[4], p[5], p[6], Double.parseDouble(p[7]));
                e.note = p[8];
                return e;
            }
        } catch (NumberFormatException ignored) {
            // a damaged line
        }
        return null;
    }

}
