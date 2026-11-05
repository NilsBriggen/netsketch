package org.netsketch.survey;

import android.os.SystemClock;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * The screens beside the survey: every network around you (grouped into access points, with a short
 * signal history), "Watch my Wi-Fi" (a timeline and what happened), the check-up (the assistant's
 * findings) and the devices on your network.
 */
final class Pages {
    static final int SURVEY = 0, AROUND = 1, WATCH = 2, CHECK = 3, DEVICES = 4;
    /** Android's foreground budget is 4 scans per 2 minutes: one every 30 s while the list is open. */
    private static final long AROUND_EVERY_MS = 30_000;
    private static final long RECENT_MS = 5 * 60_000;

    private final MainActivity a;
    final LinearLayout around, watch, check, devices;
    int page;

    private TextView aroundStatus;
    private LinearLayout aroundList;
    private int aroundMax = 15;
    private long lastAroundScan;

    private Button watchButton;
    private TextView watchStatus, watchNote;
    private Ui.Timeline timeline;
    private LinearLayout eventList;
    private long span = 3600_000;
    private int shownEvents = -1;

    private Button checkButton;
    private TextView checkStatus;
    private LinearLayout findingList;
    private Assistant.Facts facts;
    private boolean checking, finding;

    private final List<Button> sendButtons = new ArrayList<>();
    private Button findButton;
    private TextView devStatus;
    private LinearLayout devList;

    Pages(MainActivity a) {
        this.a = a;
        around = buildAround();
        watch = buildWatch();
        check = buildCheck();
        devices = buildDevices();
    }

    private int dp(float v) {
        return Ui.dp(a, v);
    }

    private LinearLayout page() {
        LinearLayout l = new LinearLayout(a);
        l.setOrientation(LinearLayout.VERTICAL);
        l.setVisibility(View.GONE);
        return l;
    }

    private static LinearLayout.LayoutParams below(int top) {
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.topMargin = top;
        return lp;
    }

    private TextView note(String s) {
        TextView t = Ui.text(a, s, 12, Ui.DIM);
        t.setPadding(dp(4), dp(12), dp(4), 0);
        return t;
    }

    /** The row of "send" and an extra action at the bottom of a page. */
    private LinearLayout sendRow(Button extra) {
        LinearLayout row = new LinearLayout(a);
        row.setPadding(0, dp(12), 0, 0);
        Button send = Ui.button(a, a.linked() ? T.t("Send to NetSketch") : T.t("Save & share file"), false);
        send.setOnClickListener(v -> a.sendOrShare());
        sendButtons.add(send);
        row.addView(send, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        if (extra != null) {
            row.addView(Ui.gap(a, 0), new LinearLayout.LayoutParams(dp(8), 1));
            row.addView(extra);
        }
        return row;
    }

    /** Switches pages; the survey page belongs to MainActivity. */
    void show(int p) {
        page = p;
        for (Button b : sendButtons) b.setText(a.linked() ? T.t("Send to NetSketch") : T.t("Save & share file"));
        around.setVisibility(p == AROUND ? View.VISIBLE : View.GONE);
        watch.setVisibility(p == WATCH ? View.VISIBLE : View.GONE);
        check.setVisibility(p == CHECK ? View.VISIBLE : View.GONE);
        devices.setVisibility(p == DEVICES ? View.VISIBLE : View.GONE);
        if (p == AROUND) {
            if (a.askPermissions(2)) drawAround();
            else aroundStatus.setText(T.t("Allow nearby networks to see what's around you."));
        }
        if (p == WATCH) drawWatch(true);
        if (p == CHECK) drawFindings();
    }

    /** Once a second from the screen's clock. */
    void tick() {
        Watch w = Watch.get();
        // Watching without the foreground service: keep the screen (and so the app) on.
        if (w.active && !WatchService.foreground) a.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        else if (!a.measuring) a.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        if (page == AROUND) aroundTick();
        if (page == WATCH) drawWatch(false);
    }

    /** A scan arrived (the screen fed it to the tracker already). */
    void scanned() {
        if (page == AROUND) drawAround();
    }

    // ---------- around you ----------

    private LinearLayout buildAround() {
        LinearLayout p = page();
        LinearLayout c = Ui.card(a);
        c.addView(Ui.heading(a, T.t("Around you")));
        c.addView(Ui.text(a, T.t("Every network the phone hears, grouped into the boxes that send them. Yours are marked. The list refreshes while it's open."), 13, Ui.DIM));
        aroundStatus = Ui.text(a, "", 13, Ui.TEXT);
        aroundStatus.setPadding(0, dp(8), 0, dp(4));
        c.addView(aroundStatus);
        aroundList = new LinearLayout(a);
        aroundList.setOrientation(LinearLayout.VERTICAL);
        c.addView(aroundList);
        p.addView(c);
        p.addView(note(T.t("k/v/r: the access point helps devices switch to a nearer one (fast roaming). RTT: it can tell the phone its distance. Android allows 4 scans every 2 minutes, so the list updates about every 30 s.")));
        return p;
    }

    private void aroundTick() {
        if (a.measuring || !a.missingPermissions().isEmpty() || !a.wifi.isWifiEnabled()) return;
        long now = SystemClock.elapsedRealtime();
        long wait = Scope.scanWaitMs(a.wifi);
        if (wait == 0 && now - lastAroundScan >= AROUND_EVERY_MS) {
            lastAroundScan = now;
            Scope.startScan(a.wifi);
        }
        long next = Math.max(wait, AROUND_EVERY_MS - (now - lastAroundScan));
        String head = aroundStatus.getTag() instanceof String ? (String) aroundStatus.getTag() : "";
        aroundStatus.setText(head + (head.isEmpty() ? "" : "\n") + T.f("Next scan in %d s", Math.max(1, (int) Math.ceil(next / 1000.0))));
    }

    void drawAround() {
        aroundList.removeAllViews();
        Survey.Connected c = a.connected();
        List<Air.Ap> aps = Air.TRACKER.since(System.currentTimeMillis() - RECENT_MS);
        List<Air.Box> boxes = Air.group(aps, a.mine, c == null ? "" : c.ssid, c == null ? "" : c.bssid);
        String head = aps.isEmpty() ? T.t("Nothing heard yet: the first scan is on its way.") : T.f("%d access points · %d networks heard in the last 5 minutes", boxes.size(), aps.size());
        aroundStatus.setTag(head);
        aroundStatus.setText(head);
        for (int i = 0; i < Math.min(aroundMax, boxes.size()); i++) aroundList.addView(boxView(boxes.get(i), c));
        if (boxes.size() > aroundMax) {
            Button more = Ui.button(a, T.f("Show all %d", boxes.size()), false);
            more.setOnClickListener(v -> {
                aroundMax = Integer.MAX_VALUE;
                drawAround();
            });
            aroundList.addView(more, below(dp(6)));
        }
    }

    private View boxView(Air.Box b, Survey.Connected c) {
        LinearLayout v = new LinearLayout(a);
        v.setOrientation(LinearLayout.VERTICAL);
        v.setPadding(0, dp(10), 0, dp(6));
        LinearLayout head = new LinearLayout(a);
        head.setGravity(Gravity.CENTER_VERTICAL);
        String names = String.join(" / ", b.names());
        TextView title = Ui.title(a, (names.isEmpty() ? T.t("(hidden)") : names) + (b.own ? T.t("  · yours") : ""), 15);
        if (b.own) title.setTextColor(Ui.ACCENT);
        head.addView(title, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        TextView best = Ui.title(a, String.format(Locale.ROOT, "%.0f", b.best()), 16);
        best.setTextColor(Ui.quality(b.best()));
        head.addView(best);
        v.addView(head);
        boolean many = b.names().size() > 1;
        for (Air.Ap r : b.radios) {
            Survey.Seen s = r.last;
            LinearLayout row = new LinearLayout(a);
            row.setGravity(Gravity.CENTER_VERTICAL);
            row.setPadding(dp(10), dp(4), 0, 0);
            LinearLayout t = new LinearLayout(a);
            t.setOrientation(LinearLayout.VERTICAL);
            String sec = s.security.equals("Open") ? T.t("Open (no password)") : s.security;
            String line = (many ? (s.ssid.isEmpty() ? T.t("(hidden)") : s.ssid) + " · " : "") + T.f("%s ch %d · %d MHz · %s", Survey.bandOf(s.freq), s.channel, s.width, sec)
                    + (s.standard.isEmpty() ? "" : " · " + s.standard) + (s.streams > 0 ? " · " + T.f("%d streams", s.streams) : "");
            boolean you = c != null && s.bssid.equalsIgnoreCase(c.bssid);
            TextView l1 = Ui.text(a, line + (you ? T.t("  · you") : ""), 12, Ui.TEXT);
            t.addView(l1);
            List<String> flags = new ArrayList<>();
            String kvr = (s.k ? "k/" : "") + (s.v ? "v/" : "") + (s.r ? "r/" : "");
            if (!kvr.isEmpty()) flags.add(T.f("fast roaming %s", kvr.substring(0, kvr.length() - 1)));
            if (s.wps) flags.add(T.t("WPS on"));
            if (r.distanceM != null) flags.add(T.f("%.1f m away (RTT)", r.distanceM));
            else if (s.ftm) flags.add("RTT");
            if (s.utilization != null) flags.add(T.f("%.0f%% busy", s.utilization * 100));
            if (s.stations != null) flags.add(T.f("%d devices", s.stations));
            if (!s.country.isEmpty()) flags.add(s.country);
            flags.add(s.bssid);
            t.addView(Ui.text(a, String.join(" · ", flags), 11, Ui.DIM));
            row.addView(t, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            Ui.MiniSpark sp = new Ui.MiniSpark(a);
            sp.set(r.history());
            row.addView(sp);
            TextView dbm = Ui.text(a, String.format(Locale.ROOT, "%.0f", s.rssi), 13, Ui.quality(s.rssi));
            dbm.setGravity(Gravity.END);
            row.addView(dbm, new LinearLayout.LayoutParams(dp(36), ViewGroup.LayoutParams.WRAP_CONTENT));
            v.addView(row);
        }
        return v;
    }

    // ---------- watch my Wi-Fi ----------

    private LinearLayout buildWatch() {
        LinearLayout p = page();
        LinearLayout c = Ui.card(a);
        c.addView(Ui.heading(a, T.t("Watch my Wi-Fi")));
        c.addView(Ui.text(a, T.t("Leave the phone where the trouble happens. Every second it notes the signal, the speed of the link and how fast the router answers, and it scans whenever Android allows. It notices dropouts, delay spikes, switches between access points and routers changing channel. It keeps going with the screen off; plug the phone in for long watches."), 13, Ui.DIM));
        watchButton = Ui.button(a, T.t("Start watching"), true);
        watchButton.setMinHeight(dp(56));
        watchButton.setOnClickListener(v -> {
            if (Watch.get().active) {
                WatchService.stop(a);
                a.ui.postDelayed(() -> drawWatch(true), 300);
            } else if (a.askPermissions(4)) {
                startWatch();
            }
        });
        c.addView(watchButton, below(dp(10)));
        watchStatus = Ui.text(a, "", 13, Ui.TEXT);
        watchStatus.setPadding(0, dp(8), 0, 0);
        c.addView(watchStatus);
        watchNote = Ui.text(a, T.t("Android didn't allow watching in the background, so keep this screen open: it stays on while watching."), 12, Ui.AMBER);
        watchNote.setVisibility(View.GONE);
        c.addView(watchNote);
        LinearLayout chips = new LinearLayout(a);
        chips.setPadding(0, dp(8), 0, 0);
        long[] spans = {600_000, 3600_000, Watch.KEEP_MS};
        String[] labels = {T.t("10 min"), T.t("1 hour"), T.t("24 hours")};
        List<TextView> all = new ArrayList<>();
        for (int i = 0; i < 3; i++) {
            final long sp = spans[i];
            TextView chip = Ui.chip(a, labels[i], Ui.ACCENT, sp == span);
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            lp.rightMargin = dp(8);
            chips.addView(chip, lp);
            all.add(chip);
            chip.setOnClickListener(v -> {
                span = sp;
                for (TextView o : all) {
                    boolean on = o == chip;
                    o.setTextColor(on ? Ui.BG : Ui.ACCENT);
                    o.setBackground(Ui.round(a, on ? Ui.ACCENT : android.graphics.Color.TRANSPARENT, 20, on ? 0 : Ui.ACCENT));
                }
                drawWatch(true);
            });
        }
        c.addView(chips);
        timeline = new Ui.Timeline(a);
        timeline.setContentDescription(T.t("Signal and router delay over time"));
        LinearLayout.LayoutParams tl = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(170));
        tl.topMargin = dp(8);
        c.addView(timeline, tl);
        c.addView(Ui.text(a, T.t("Line: signal (dBm). Bars: the router's delay. Red: no answer or offline. Marks on top: events."), 11, Ui.DIM));
        p.addView(c);
        LinearLayout e = Ui.card(a);
        e.addView(Ui.heading(a, T.t("What happened")));
        eventList = new LinearLayout(a);
        eventList.setOrientation(LinearLayout.VERTICAL);
        e.addView(eventList);
        Button clear = Ui.button(a, T.t("Clear log"), false);
        clear.setOnClickListener(v -> new android.app.AlertDialog.Builder(a)
                .setMessage(T.t("Delete the watch log on this phone?"))
                .setPositiveButton(T.t("Delete"), (d, w) -> {
                    Watch.get().clear();
                    drawWatch(true);
                })
                .setNegativeButton(T.t("Keep"), null)
                .show());
        e.addView(sendRow(clear));
        p.addView(e);
        return p;
    }

    void startWatch() {
        if (android.os.Build.VERSION.SDK_INT >= 33 && a.checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            // Only for the notification; watching works without it.
            a.requestPermissions(new String[] {android.Manifest.permission.POST_NOTIFICATIONS}, 5);
        }
        try {
            WatchService.start(a);
        } catch (RuntimeException e) {
            watchStatus.setText(T.f("Couldn't start watching: %s", e.getMessage() == null ? T.t("unknown error") : e.getMessage()));
            return;
        }
        a.ui.postDelayed(() -> drawWatch(true), 500);
    }

    private void drawWatch(boolean full) {
        Watch w = Watch.get();
        boolean on = w.active;
        watchButton.setText(on ? T.t("Stop watching") : T.t("Start watching"));
        watchNote.setVisibility(on && !WatchService.foreground ? View.VISIBLE : View.GONE);
        long now = System.currentTimeMillis();
        List<Watch.Sample> s = w.samples(now - span);
        List<Survey.Event> ev = w.events();
        int n = w.size();
        if (on) {
            long mins = (now - w.startedAt) / 60_000;
            watchStatus.setText(T.f("Watching for %d h %d min · %d readings · %d events", mins / 60, mins % 60, n, ev.size()));
        } else {
            watchStatus.setText(n == 0 ? T.t("Nothing recorded yet.") : T.f("%d readings and %d events from the last 24 hours.", n, ev.size()));
        }
        timeline.samples = s;
        timeline.events = ev;
        timeline.to = now;
        timeline.from = s.isEmpty() ? now - span : Math.max(now - span, Math.min(s.get(0).at, now - 60_000));
        timeline.invalidate();
        if (!full && ev.size() == shownEvents) return;
        shownEvents = ev.size();
        eventList.removeAllViews();
        if (ev.isEmpty()) eventList.addView(Ui.text(a, T.t("No events yet."), 14, Ui.DIM));
        java.text.DateFormat tf = android.text.format.DateFormat.getTimeFormat(a);
        for (int i = ev.size() - 1, shown = 0; i >= 0 && shown < 40; i--, shown++) {
            Survey.Event e = ev.get(i);
            LinearLayout row = new LinearLayout(a);
            row.setPadding(0, dp(4), 0, dp(4));
            TextView when = Ui.text(a, tf.format(new java.util.Date(e.at)), 12, Ui.DIM);
            row.addView(when, new LinearLayout.LayoutParams(dp(64), ViewGroup.LayoutParams.WRAP_CONTENT));
            row.addView(Ui.text(a, describe(e), 14, Ui.eventColor(e.kind)), new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            eventList.addView(row);
        }
    }

    private static String tail(String bssid) {
        return bssid.length() > 8 ? "…" + bssid.substring(bssid.length() - 8) : bssid;
    }

    static String describe(Survey.Event e) {
        switch (e.kind) {
            case "channel": return T.f("'%s' moved from channel %s to %s (%s)", e.ssid.isEmpty() ? T.t("(hidden)") : e.ssid, e.from, e.to, e.note);
            case "roam": return T.f("Switched access point: %s → %s (%.0f dBm)", tail(e.from), tail(e.to), e.value);
            case "drop": return T.f("Wi-Fi dropped for %.0f s", e.value);
            case "spike": return T.f("Delay spike: %.0f ms", e.value);
            case "loss": return T.f("The router stopped answering (%.0f%% lost)", e.value);
            case "dip": return T.f("Signal dipped to %.0f dBm", e.value);
            default: return e.kind;
        }
    }

    // ---------- check-up ----------

    private LinearLayout buildCheck() {
        LinearLayout p = page();
        LinearLayout c = Ui.card(a);
        c.addView(Ui.heading(a, T.t("Check-up")));
        c.addView(Ui.text(a, T.t("What the phone can tell about your network: channels and neighbours, your access points' settings, roaming, the watch log, your measured spots and the internet side."), 13, Ui.DIM));
        checkButton = Ui.button(a, T.t("Check my network"), true);
        checkButton.setMinHeight(dp(56));
        checkButton.setOnClickListener(v -> runCheck());
        c.addView(checkButton, below(dp(10)));
        checkStatus = Ui.text(a, "", 13, Ui.DIM);
        checkStatus.setPadding(0, dp(8), 0, dp(4));
        c.addView(checkStatus);
        findingList = new LinearLayout(a);
        findingList.setOrientation(LinearLayout.VERTICAL);
        c.addView(findingList);
        c.addView(sendRow(null));
        p.addView(c);
        p.addView(note(T.t("A phone sees a lot, but not everything: it can't see which way an extender links back, what other devices experience, or the router's own error counters. Findings about that say \"likely\".")));
        return p;
    }

    private void runCheck() {
        if (checking) return;
        if (Scope.wifiNetwork(a) == null) {
            checkStatus.setText(T.t("Connect to your Wi-Fi first."));
            checkStatus.setTextColor(Ui.AMBER);
            drawFindings();
            return;
        }
        checking = true;
        checkButton.setEnabled(false);
        checkButton.setAlpha(0.5f);
        checkStatus.setText(T.t("Checking the router and the internet side… about 10 seconds."));
        checkStatus.setTextColor(Ui.DIM);
        new Thread(() -> {
            Assistant.Facts f = Scope.facts(a);
            a.ui.post(() -> {
                checking = false;
                checkButton.setEnabled(true);
                checkButton.setAlpha(1f);
                facts = f;
                checkStatus.setText("");
                drawFindings();
                if (a.linked()) a.sendUnsent(false);
            });
        }).start();
    }

    /** Runs the assistant on everything known and keeps its findings for the file. */
    List<Survey.Finding> findings() {
        Assistant.Input in = new Assistant.Input();
        in.aps = Air.TRACKER.since(System.currentTimeMillis() - RECENT_MS);
        in.connected = a.connected();
        in.mine = a.mine;
        in.events = Assistant.merge(Watch.get().events(), Air.TRACKER.changes());
        in.points = a.survey.points;
        in.facts = facts;
        List<Survey.Finding> f = Assistant.run(in);
        a.survey.findings.clear();
        a.survey.findings.addAll(f);
        a.save();
        return f;
    }

    private void drawFindings() {
        findingList.removeAllViews();
        List<Survey.Finding> f = findings();
        if (f.isEmpty()) {
            findingList.addView(Ui.text(a, T.t("Nothing to fix found. Watch for a while, look around, or measure a few spots: then there is more to go on."), 14, Ui.GREEN));
        }
        for (Survey.Finding x : f) {
            LinearLayout row = new LinearLayout(a);
            row.setOrientation(LinearLayout.VERTICAL);
            row.setPadding(0, dp(6), 0, dp(6));
            int col = x.level.equals("error") ? Ui.RED : x.level.equals("warning") ? Ui.AMBER : Ui.ACCENT;
            String word = x.level.equals("error") ? T.t("Problem") : x.level.equals("warning") ? T.t("Worth a look") : T.t("Tip");
            TextView lvl = Ui.text(a, word.toUpperCase(Locale.getDefault()), 11, col);
            lvl.setTypeface(Ui.MEDIUM);
            row.addView(lvl);
            row.addView(Ui.text(a, x.text, 14, Ui.TEXT));
            findingList.addView(row);
        }
        if (facts == null) checkStatus.setText(T.t("Press the button to also check the router and the internet side."));
    }

    // ---------- devices ----------

    private LinearLayout buildDevices() {
        LinearLayout p = page();
        LinearLayout c = Ui.card(a);
        c.addView(Ui.heading(a, T.t("Devices on your network")));
        c.addView(Ui.text(a, T.t("Finds what's connected: devices that announce themselves (TVs, printers, speakers, the router itself) and any address that answers a knock."), 13, Ui.DIM));
        findButton = Ui.button(a, T.t("Find devices"), true);
        findButton.setMinHeight(dp(56));
        findButton.setOnClickListener(v -> findDevices());
        c.addView(findButton, below(dp(10)));
        devStatus = Ui.text(a, "", 13, Ui.DIM);
        devStatus.setPadding(0, dp(8), 0, dp(4));
        c.addView(devStatus);
        devList = new LinearLayout(a);
        devList.setOrientation(LinearLayout.VERTICAL);
        c.addView(devList);
        c.addView(sendRow(null));
        p.addView(c);
        p.addView(note(T.t("Android hides other devices' hardware addresses from apps, so a device that doesn't announce itself shows only its address. Phones often ignore knocks and may be missing.")));
        drawDevices();
        return p;
    }

    private void findDevices() {
        if (finding) return;
        if (Scope.wifiNetwork(a) == null) {
            devStatus.setText(T.t("Connect to your Wi-Fi first."));
            devStatus.setTextColor(Ui.AMBER);
            return;
        }
        finding = true;
        findButton.setEnabled(false);
        findButton.setAlpha(0.5f);
        devStatus.setTextColor(Ui.DIM);
        new Thread(() -> {
            List<Survey.LanDevice> l = Scope.devices(a, s -> a.ui.post(() -> devStatus.setText(s)));
            a.ui.post(() -> {
                finding = false;
                findButton.setEnabled(true);
                findButton.setAlpha(1f);
                a.survey.lan.clear();
                a.survey.lan.addAll(l);
                a.save();
                drawDevices();
                if (a.linked()) a.sendUnsent(false);
            });
        }).start();
    }

    private void drawDevices() {
        devList.removeAllViews();
        List<Survey.LanDevice> l = a.survey.lan;
        devStatus.setText(l.isEmpty() ? T.t("Nothing searched yet.") : T.f("%d devices found.", l.size()));
        for (Survey.LanDevice d : l) {
            LinearLayout row = new LinearLayout(a);
            row.setOrientation(LinearLayout.VERTICAL);
            row.setPadding(0, dp(6), 0, dp(6));
            row.addView(Ui.title(a, d.name.isEmpty() ? T.t("Unknown device") : d.name, 15));
            List<String> parts = new ArrayList<>();
            if (!d.kind.isEmpty()) parts.add(T.t(d.kind));
            String make = (d.vendor + " " + d.model).trim();
            if (!make.isEmpty() && !make.equals(d.name)) parts.add(make);
            parts.add(d.ip);
            row.addView(Ui.text(a, String.join(" · ", parts), 12, Ui.DIM));
            devList.addView(row);
        }
    }
}
