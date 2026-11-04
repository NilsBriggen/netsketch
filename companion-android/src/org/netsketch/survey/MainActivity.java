package org.netsketch.survey;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
import android.content.BroadcastReceiver;
import android.content.ContentValues;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.content.res.Configuration;
import android.location.LocationManager;
import android.net.ConnectivityManager;
import android.net.LinkProperties;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.net.RouteInfo;
import android.net.Uri;
import android.net.wifi.ScanResult;
import android.net.wifi.WifiInfo;
import android.net.wifi.WifiManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.provider.MediaStore;
import android.provider.Settings;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.Gravity;
import android.view.HapticFeedbackConstants;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.EditText;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.File;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.ConnectException;
import java.net.HttpURLConnection;
import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.net.URL;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.text.SimpleDateFormat;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Date;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * NetSketch Survey. Link it to NetSketch on your computer (it finds it on the Wi-Fi), tap where you stand
 * on your floor plan, press Measure: every network heard there — and, if you like, the real speed — lands
 * in NetSketch as a pin in the right place. Without a computer nearby it keeps the points and shares them
 * as a file.
 */
public class MainActivity extends Activity {
    private static final int ROUNDS = 3;
    /** Android 9+ lets an app in the foreground start 4 Wi-Fi scans every 2 minutes. */
    private static final int SCAN_BUDGET = 4;
    private static final long SCAN_WINDOW_MS = 120_000;
    /** Shown in the list before "Show all". */
    private static final int SHORT_LIST = 12;
    /** Readings of a quick point (every 0.5 s) and knocks of the ping test. */
    private static final int QUICK_TICKS = 7, PINGS = 20;
    /** Scan starts survive the screen turning (they are Android's count, not the screen's). */
    private static final List<Long> scanTimes = new ArrayList<>();

    private final Handler ui = new Handler(Looper.getMainLooper());
    private Survey survey = new Survey();
    private WifiManager wifi;
    private SharedPreferences prefs;
    private Link link;
    /** Whether the last call to NetSketch got through; failed plan fetches in a row (3 = look for the computer again). */
    private boolean linkOk, rediscovering;
    private int fetchFails;
    /** Network names of the person's own access points (from the plan), kept for when not linked. */
    private final Set<String> mine = new HashSet<>();
    private String planJson;
    private final List<PlanView.Pin> computerPins = new ArrayList<>();

    // Screen parts that change.
    private ScrollView scroll;
    private TextView linkChip, liveDbm, liveWord, liveName, liveRange, measureStatus, resultsTitle, pointsTitle, planHint, summary;
    private Ui.Bars liveBars;
    private Ui.Spark spark;
    private EditText nameField, floorField;
    private Switch speedSwitch;
    private Button measureButton, quickButton, pingButton, sendButton, clearButton, linkButton, settingsButton;
    private LinearLayout planCard, storeyRow, results, resultsCard, pointList, floorRow, throttleTip, measureCard;
    private PlanView plan;
    private boolean nameTyped, settingName;

    // The measurement in progress, and the point it replaces ("Measure again here").
    private Survey.Point current, again;
    /** The point whose speed test may go on; cleared to stop it. */
    private volatile Survey.Point testing;
    private boolean quick, pinging;
    private int quickReadings;
    private double quickRssi, quickLink;
    private Map<String, Survey.Seen> heard;
    private int round, failedStarts;
    private long roundStart, waitUntil;
    private boolean measuring, limited;
    /** A scan this app started is still out: only then does a "results ready" broadcast count (Android sends them for its own scans too). */
    private boolean scanPending;
    private BroadcastReceiver receiver;
    private final Runnable roundTimeout = this::collectRound, nextRound = this::scanRound;
    /** Counts down until Android allows a fresh scan, then carries on; the person stays where they are. */
    private final Runnable countdown = new Runnable() {
        @Override
        public void run() {
            if (!measuring) return;
            long left = waitUntil - SystemClock.elapsedRealtime();
            if (left <= 0) {
                scanRound();
                return;
            }
            int s = (int) Math.ceil(left / 1000.0);
            measureStatus.setText(T.f("Android allows 4 Wi-Fi scans every 2 minutes. Fresh scan in %d s — stay where you are.", s));
            measureStatus.setTextColor(Ui.AMBER);
            ui.postDelayed(this, 1000);
        }
    };
    /** A quick point: the connection's own readings, twice a second for about 3 s (no scan). */
    private final Runnable quickTick = new Runnable() {
        @Override
        public void run() {
            if (!measuring) return;
            Survey.Connected c = connected();
            if (c != null) {
                quickRssi += c.rssi;
                quickLink += c.linkMbps;
                quickReadings++;
            }
            if (++round < QUICK_TICKS) {
                measureStatus.setText(T.f("Reading your Wi-Fi… %d s. Stand still.", (QUICK_TICKS - round + 1) / 2));
                ui.postDelayed(this, 500);
            } else {
                finishQuick();
            }
        }
    };
    /** One send at a time: after a point, from the Send button and from the plan refresh. */
    private final AtomicBoolean sending = new AtomicBoolean();
    private int ticks;
    private final Runnable live = new Runnable() {
        @Override
        public void run() {
            showLive();
            // Fetch the plan again now and then: renames, new walls, and whether the link is still on.
            if (++ticks % 30 == 0 && !measuring && link != null && !link.code.isEmpty()) fetchPlan(false);
            ui.postDelayed(this, 1000);
        }
    };

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Ui.init(this);
        requestWindowFeature(Window.FEATURE_NO_TITLE);
        getWindow().setStatusBarColor(Ui.BG);
        getWindow().setNavigationBarColor(Ui.BG);
        wifi = getApplicationContext().getSystemService(WifiManager.class);
        prefs = getSharedPreferences("link", MODE_PRIVATE);
        mine.addAll(prefs.getStringSet("networks", Collections.emptySet()));
        load();
        buildUi();
        if (Build.VERSION.SDK_INT >= 30 && !Ui.dark) {
            WindowInsetsController c = getWindow().getInsetsController();
            int light = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
            if (c != null) c.setSystemBarsAppearance(light, light);
        } else if (!Ui.dark) {
            lightBarsAndroid10();
        }
        refreshPoints();
        // Reconnect to the computer used last time, quietly.
        String host = prefs.getString("host", null);
        if (host != null) {
            link = new Link(host, prefs.getInt("port", Link.DEFAULT_PORT), prefs.getString("code", ""), prefs.getString("title", ""));
            fetchPlan(false);
        }
        showLink();
    }

    /** Dark status and navigation bar icons on the light background (Android 10 has no insets controller yet). */
    @SuppressWarnings("deprecation")
    private void lightBarsAndroid10() {
        getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
    }

    @Override
    protected void onResume() {
        super.onResume();
        ui.post(live);
        // Back from the app settings with the permission given.
        if (missingPermissions().isEmpty()) settingsButton.setVisibility(View.GONE);
    }

    @Override
    public void onConfigurationChanged(Configuration c) {
        super.onConfigurationChanged(c);
        // The colours are picked in onCreate, so dark/light mode builds the screen again (the points are
        // saved, the link comes back by itself). Not in the middle of a measurement: the next start picks it up.
        boolean night = (c.uiMode & Configuration.UI_MODE_NIGHT_MASK) != Configuration.UI_MODE_NIGHT_NO;
        if (night != Ui.dark && !measuring && !sending.get() && !pinging) {
            recreate();
            return;
        }
        // Turned the phone: the plan keeps its share of the new height.
        plan.getLayoutParams().height = planHeight();
        plan.requestLayout();
    }

    @Override
    protected void onPause() {
        ui.removeCallbacks(live);
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        stopListening();
        testing = null; // stops a running speed test
        ui.removeCallbacksAndMessages(null);
        super.onDestroy();
    }

    // ---------- screen ----------

    private int dp(float v) {
        return Ui.dp(this, v);
    }

    private static LinearLayout.LayoutParams below(int top) {
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.topMargin = top;
        return lp;
    }

    private void buildUi() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(16), dp(12), dp(16), dp(24));
        root.setBackgroundColor(Ui.BG);

        // Header: name and link status.
        LinearLayout head = new LinearLayout(this);
        head.setGravity(Gravity.CENTER_VERTICAL);
        LinearLayout names = new LinearLayout(this);
        names.setOrientation(LinearLayout.VERTICAL);
        names.addView(Ui.title(this, "NetSketch Survey", 22));
        names.addView(Ui.text(this, T.t("Your real Wi-Fi, spot by spot"), 13, Ui.DIM));
        head.addView(names, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        linkChip = Ui.chip(this, T.t("Link"), Ui.DIM, false);
        linkChip.setSingleLine(true);
        linkChip.setEllipsize(android.text.TextUtils.TruncateAt.END);
        linkChip.setMaxWidth(dp(170));
        linkChip.setOnClickListener(v -> linkDialog());
        head.addView(linkChip);
        root.addView(head);

        // Live signal of the phone's own Wi-Fi, and its last minute.
        LinearLayout liveCard = Ui.card(this);
        liveCard.addView(Ui.heading(this, T.t("Your Wi-Fi right here")));
        LinearLayout liveRow = new LinearLayout(this);
        liveRow.setGravity(Gravity.CENTER_VERTICAL);
        liveDbm = Ui.title(this, "—", 34);
        liveRow.addView(liveDbm);
        LinearLayout liveText = new LinearLayout(this);
        liveText.setOrientation(LinearLayout.VERTICAL);
        liveText.setPadding(dp(12), 0, 0, 0);
        liveWord = Ui.title(this, "", 16);
        liveName = Ui.text(this, "", 13, Ui.DIM);
        liveText.addView(liveWord);
        liveText.addView(liveName);
        liveRow.addView(liveText, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        liveBars = new Ui.Bars(this);
        liveRow.addView(liveBars);
        liveCard.addView(liveRow);
        spark = new Ui.Spark(this);
        LinearLayout.LayoutParams sp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(64));
        sp.topMargin = dp(10);
        liveCard.addView(spark, sp);
        liveRange = Ui.text(this, "", 12, Ui.DIM);
        liveCard.addView(liveRange);
        root.addView(liveCard);

        // Floor plan (from NetSketch) — tap where you stand.
        planCard = Ui.card(this);
        planCard.addView(Ui.heading(this, T.t("Where are you?")));
        HorizontalScrollView storeyScroll = new HorizontalScrollView(this);
        storeyScroll.setHorizontalScrollBarEnabled(false);
        storeyRow = new LinearLayout(this);
        storeyScroll.addView(storeyRow);
        planCard.addView(storeyScroll);
        plan = new PlanView(this);
        LinearLayout.LayoutParams plp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, planHeight());
        plp.topMargin = dp(8);
        planCard.addView(plan, plp);
        plan.onSpot = (x, y) -> {
            again = null; // a new spot: a new point, not the old one measured again
            String room = plan.roomAt(x, y);
            if (room != null && !nameTyped) setName(room);
            planHint.setText(T.f("Spot picked: %.1f m, %.1f m — measure when you're standing there.", x, y));
        };
        plan.onPin = pin -> {
            if (pin.tag instanceof Survey.Point) showAndScroll((Survey.Point) pin.tag);
        };
        planHint = Ui.text(this, T.t("Link to NetSketch on your computer to tap your spot on the floor plan."), 13, Ui.DIM);
        planHint.setPadding(0, dp(8), 0, 0);
        planCard.addView(planHint);
        linkButton = Ui.button(this, T.t("Link to NetSketch"), false);
        linkButton.setOnClickListener(v -> linkDialog());
        LinearLayout.LayoutParams llp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        llp.topMargin = dp(10);
        planCard.addView(linkButton, llp);
        root.addView(planCard);

        // Name, storey, speed test, and the big button.
        measureCard = Ui.card(this);
        LinearLayout m = measureCard;
        m.addView(Ui.heading(this, T.t("Measure")));
        nameField = Ui.field(this, T.t("Name this spot, e.g. Sofa"));
        nameField.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int a, int b, int c) {}

            @Override
            public void onTextChanged(CharSequence s, int a, int b, int c) {}

            @Override
            public void afterTextChanged(Editable e) {
                // Typed by the person (not filled in from the room name): keep it.
                if (!settingName) nameTyped = e.length() > 0;
            }
        });
        m.addView(nameField);
        floorRow = new LinearLayout(this);
        floorRow.setGravity(Gravity.CENTER_VERTICAL);
        floorRow.setPadding(0, dp(10), 0, 0);
        floorRow.addView(Ui.text(this, T.t("Storey  "), 15, Ui.TEXT));
        floorField = Ui.field(this, "0");
        floorField.setInputType(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_SIGNED);
        floorField.setText("0");
        floorField.setEms(3);
        floorRow.addView(floorField);
        floorRow.addView(Ui.text(this, T.t("   0 = ground, −1 = basement"), 13, Ui.DIM));
        m.addView(floorRow);
        speedSwitch = new Switch(this);
        speedSwitch.setText(T.t("Speed test too (uses about 35 MB)"));
        speedSwitch.setTextColor(Ui.TEXT);
        speedSwitch.setTextSize(15);
        speedSwitch.setPadding(0, dp(12), 0, dp(4));
        speedSwitch.setChecked(prefs.getBoolean("speed", false));
        speedSwitch.setOnCheckedChangeListener((b, on) -> prefs.edit().putBoolean("speed", on).apply());
        m.addView(speedSwitch);
        measureButton = Ui.button(this, T.t("Measure here"), true);
        measureButton.setTextSize(18);
        // Grows with the text (large font sizes wrap it) instead of a fixed height that cuts it off.
        measureButton.setMinHeight(dp(60));
        m.addView(measureButton, below(dp(10)));
        measureButton.setOnClickListener(v -> {
            if (measuring) cancelMeasuring();
            else startMeasuring(false);
        });
        // The quick point (no scan, so it doesn't use up Android's scans) and the delay to the router.
        LinearLayout extra = new LinearLayout(this);
        quickButton = Ui.button(this, T.t("Quick point (signal only)"), false);
        quickButton.setOnClickListener(v -> startMeasuring(true));
        extra.addView(quickButton, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 3f));
        extra.addView(Ui.gap(this, 0), new LinearLayout.LayoutParams(dp(8), 1));
        pingButton = Ui.button(this, T.t("Ping test"), false);
        pingButton.setOnClickListener(v -> pingTest());
        extra.addView(pingButton, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 2f));
        m.addView(extra, below(dp(8)));
        measureStatus = Ui.text(this, T.t("Stand still for about 10 seconds while it listens."), 13, Ui.DIM);
        measureStatus.setPadding(0, dp(8), 0, 0);
        m.addView(measureStatus);
        // Shown once Android no longer asks for the permission itself ("Don't ask again").
        settingsButton = Ui.button(this, T.t("Open app settings"), false);
        settingsButton.setOnClickListener(v -> startActivity(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", getPackageName(), null))));
        settingsButton.setVisibility(View.GONE);
        LinearLayout.LayoutParams slp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        slp.topMargin = dp(8);
        m.addView(settingsButton, slp);
        // Shown once Android's scan limit got in the way.
        throttleTip = new LinearLayout(this);
        throttleTip.setGravity(Gravity.CENTER_VERTICAL);
        throttleTip.setPadding(0, dp(8), 0, 0);
        throttleTip.addView(Ui.text(this, T.t("Faster surveys: turn off \"Wi-Fi scan throttling\" in Developer options."), 12, Ui.DIM), new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        Button dev = Ui.button(this, T.t("Open"), false);
        dev.setOnClickListener(v -> {
            try {
                startActivity(new Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS));
            } catch (ActivityNotFoundException e) {
                startActivity(new Intent(Settings.ACTION_SETTINGS));
            }
        });
        throttleTip.addView(dev);
        throttleTip.setVisibility(View.GONE);
        m.addView(throttleTip);
        root.addView(m);

        // What was heard at the last (or a chosen) point.
        resultsCard = Ui.card(this);
        resultsTitle = Ui.heading(this, T.t("Networks here"));
        resultsCard.addView(resultsTitle);
        results = new LinearLayout(this);
        results.setOrientation(LinearLayout.VERTICAL);
        resultsCard.addView(results);
        resultsCard.setVisibility(View.GONE);
        root.addView(resultsCard);

        // All points, and getting them to NetSketch.
        LinearLayout pc = Ui.card(this);
        pointsTitle = Ui.heading(this, T.t("Points"));
        pc.addView(pointsTitle);
        summary = Ui.text(this, "", 13, Ui.TEXT);
        summary.setPadding(0, 0, 0, dp(6));
        pc.addView(summary);
        pointList = new LinearLayout(this);
        pointList.setOrientation(LinearLayout.VERTICAL);
        pc.addView(pointList);
        LinearLayout actions = new LinearLayout(this);
        actions.setPadding(0, dp(12), 0, 0);
        sendButton = Ui.button(this, T.t("Send to NetSketch"), false);
        sendButton.setOnClickListener(v -> sendOrShare());
        actions.addView(sendButton, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        actions.addView(Ui.gap(this, 0), new LinearLayout.LayoutParams(dp(8), 1));
        clearButton = Ui.button(this, T.t("Start over"), false);
        clearButton.setOnClickListener(v -> new AlertDialog.Builder(this)
                .setMessage(T.f("Delete all %d points on this phone? Points already in NetSketch stay there.", survey.points.size()))
                .setPositiveButton(T.t("Delete"), (d, w) -> {
                    survey.points.clear();
                    save();
                    resultsCard.setVisibility(View.GONE);
                    refreshPoints();
                })
                .setNegativeButton(T.t("Keep"), null)
                .show());
        actions.addView(clearButton);
        pc.addView(actions);
        root.addView(pc);

        TextView how = Ui.text(this, T.t("Tip: measure where you actually use Wi-Fi (desk, sofa, bed), the far corners, and near the walls to your neighbours. "
                + "Networks you don't own can then be added as neighbours in NetSketch with one click, with the channels and busy time you measured."), 12, Ui.DIM);
        how.setPadding(dp(4), dp(16), dp(4), 0);
        root.addView(how);
        String version = "";
        try {
            version = getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (PackageManager.NameNotFoundException ignored) {
            // can't happen for the app itself
        }
        TextView about = Ui.text(this, T.f("NetSketch Survey %s · GPL-3.0", version), 11, Ui.DIM);
        about.setPadding(dp(4), dp(12), dp(4), 0);
        root.addView(about);

        scroll = new ScrollView(this);
        scroll.setBackgroundColor(Ui.BG);
        scroll.setFillViewport(true);
        scroll.setClipToPadding(false);
        // Android 15 draws apps under the status and navigation bars: leave room for them.
        scroll.setOnApplyWindowInsetsListener((v, in) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets b = in.getInsets(android.view.WindowInsets.Type.systemBars() | android.view.WindowInsets.Type.displayCutout() | android.view.WindowInsets.Type.ime());
                v.setPadding(b.left, b.top, b.right, b.bottom);
            } else {
                v.setPadding(in.getSystemWindowInsetLeft(), in.getSystemWindowInsetTop(), in.getSystemWindowInsetRight(), in.getSystemWindowInsetBottom());
            }
            return in;
        });
        scroll.addView(root);
        setContentView(scroll);
    }

    /** The floor plan takes 45% of the screen's height (at least 240 dp). */
    private int planHeight() {
        return Math.max(dp(240), Math.round(getResources().getDisplayMetrics().heightPixels * 0.45f));
    }

    private void showAndScroll(Survey.Point p) {
        showResults(p);
        ui.post(() -> scroll.smoothScrollTo(0, resultsCard.getTop()));
    }

    private void setName(String name) {
        settingName = true;
        nameField.setText(name);
        settingName = false;
    }

    /** The live signal card, once a second. */
    private void showLive() {
        Survey.Connected c = connected();
        if (c == null) {
            liveDbm.setText("—");
            liveDbm.setTextColor(Ui.DIM);
            liveWord.setText(T.t("Not on Wi-Fi"));
            liveName.setText(T.t("Connect to your Wi-Fi to see its strength here."));
            liveBars.set(-100);
            liveRange.setText("");
            return;
        }
        liveDbm.setText(String.format(Locale.ROOT, "%.0f", c.rssi));
        liveDbm.setTextColor(Ui.quality(c.rssi));
        liveWord.setText(T.f("dBm · %s", Ui.qualityWord(c.rssi)));
        liveName.setText(T.f("%s · %s ch %d · %.0f Mbit/s link", c.ssid.isEmpty() ? T.t("name hidden until you allow nearby networks") : c.ssid, Survey.bandOf(c.freq), Survey.channelOf(c.freq), c.linkMbps));
        liveBars.set(c.rssi);
        spark.add(c.rssi);
        float[] r = spark.range();
        liveRange.setText(r == null ? "" : T.f("last minute: %.0f to %.0f dBm", r[0], r[1]));
    }

    private void showLink() {
        boolean known = link != null && !link.code.isEmpty();
        boolean on = known && linkOk && !plan.walls.isEmpty();
        String waiting = fetchFails >= 3 ? T.t("Not reachable") : T.t("Linking…");
        linkChip.setText(on ? T.f("Linked · %s", link.title.isEmpty() ? link.host : link.title) : known ? waiting : T.t("Link"));
        linkChip.setTextColor(on ? Ui.BG : known ? Ui.AMBER : Ui.ACCENT);
        linkChip.setBackground(Ui.round(this, on ? Ui.ACCENT : android.graphics.Color.TRANSPARENT, 20, on ? 0 : known ? Ui.AMBER : Ui.ACCENT));
        floorRow.setVisibility(plan.walls.isEmpty() ? View.VISIBLE : View.GONE);
        storeyRow.setVisibility(plan.walls.isEmpty() ? View.GONE : View.VISIBLE);
        plan.setVisibility(plan.walls.isEmpty() ? View.GONE : View.VISIBLE);
        sendButton.setText(known ? T.t("Send to NetSketch") : T.t("Save & share file"));
        linkButton.setVisibility(known ? View.GONE : View.VISIBLE);
        if (!known) planHint.setText(T.t("Link to NetSketch on your computer to tap your spot on the floor plan."));
    }

    /** Your own Wi-Fi's strength at a point: the phone's connection, else the strongest of your networks. */
    private double ownRssi(Survey.Point p) {
        if (p.connected != null) return p.connected.rssi;
        double best = Double.NaN;
        for (Survey.Seen s : p.networks) if (Survey.isOwn(p, s, mine) && (Double.isNaN(best) || s.rssi > best)) best = s.rssi;
        return best;
    }

    private void refreshPoints() {
        pointList.removeAllViews();
        int unsent = 0;
        for (Survey.Point p : survey.points) if (!p.sent) unsent++;
        String title = T.f("Points · %d", survey.points.size()) + (unsent > 0 && link != null ? T.f(" · %d not sent yet", unsent) : "");
        pointsTitle.setText(title.toUpperCase(Locale.getDefault()));
        if (survey.points.isEmpty()) {
            pointList.addView(Ui.text(this, T.t("None yet. Measure your first spot above."), 14, Ui.DIM));
        }
        clearButton.setVisibility(survey.points.isEmpty() ? View.GONE : View.VISIBLE);
        sendButton.setEnabled(!survey.points.isEmpty());
        sendButton.setAlpha(survey.points.isEmpty() ? 0.5f : 1f);
        // One line over all points: how far your Wi-Fi's strength (and speed) spreads.
        double lo = Double.MAX_VALUE, hi = -Double.MAX_VALUE, slow = Double.MAX_VALUE, fast = 0;
        int weak = 0, known = 0;
        for (Survey.Point p : survey.points) {
            double r = ownRssi(p);
            if (!Double.isNaN(r)) {
                known++;
                lo = Math.min(lo, r);
                hi = Math.max(hi, r);
                if (r < -70) weak++;
            }
            if (p.speed != null) {
                slow = Math.min(slow, p.speed.downMbps);
                fast = Math.max(fast, p.speed.downMbps);
            }
        }
        String sum = known < 2 ? "" : T.f("Your Wi-Fi over %d spots: %.0f to %.0f dBm, %d weak.", known, hi, lo, weak);
        if (fast > 0 && slow < fast) sum += (sum.isEmpty() ? "" : " ") + T.f("Fastest %.0f, slowest %.0f Mbit/s.", fast, slow);
        summary.setText(sum);
        summary.setVisibility(sum.isEmpty() ? View.GONE : View.VISIBLE);
        for (int i = survey.points.size() - 1; i >= 0; i--) {
            final Survey.Point p = survey.points.get(i);
            LinearLayout row = new LinearLayout(this);
            row.setGravity(Gravity.CENTER_VERTICAL);
            row.setPadding(0, dp(6), 0, dp(6));
            Ui.Bars bars = new Ui.Bars(this);
            double r = ownRssi(p);
            bars.set(Double.isNaN(r) ? -100 : r);
            row.addView(bars);
            LinearLayout t = new LinearLayout(this);
            t.setOrientation(LinearLayout.VERTICAL);
            t.setPadding(dp(12), dp(4), 0, dp(4));
            t.addView(Ui.title(this, p.name, 15));
            String where = p.x != null ? T.t("placed on the plan") : T.t("pin to place on the computer");
            String speed = p.speed == null ? "" : T.f(" · %.0f Mbit/s", p.speed.downMbps);
            String heardHere = p.scans == 0 ? T.f("Signal only · %s", where) : T.f("%d networks · %s", p.networks.size(), where);
            t.addView(Ui.text(this, heardHere + speed, 12, Ui.DIM));
            t.setOnClickListener(v -> showAndScroll(p));
            t.setOnLongClickListener(v -> {
                pointMenu(p);
                return true;
            });
            row.addView(t, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            row.addView(Ui.chip(this, p.sent ? T.t("In NetSketch") : T.t("On phone"), p.sent ? Ui.GREEN : Ui.DIM, false));
            TextView more = Ui.text(this, "⋮", 20, Ui.DIM);
            more.setGravity(Gravity.CENTER);
            more.setContentDescription(T.t("Rename") + ", " + T.t("Delete"));
            more.setOnClickListener(v -> pointMenu(p));
            row.addView(more, new LinearLayout.LayoutParams(dp(48), dp(48)));
            pointList.addView(row);
        }
        // Points on the plan: hollow ones NetSketch has, filled ones from this phone coloured by your Wi-Fi.
        plan.pins.clear();
        plan.pins.addAll(computerPins);
        for (Survey.Point p : survey.points) {
            if (p.x == null) continue;
            PlanView.Pin q = new PlanView.Pin();
            q.floor = p.floor;
            q.x = p.x.floatValue();
            q.y = p.y.floatValue();
            q.name = p.name;
            q.rssi = ownRssi(p);
            q.tag = p;
            plan.pins.add(q);
        }
        plan.invalidate();
    }

    /** Rename, measure again at the same spot, or delete one point. */
    private void pointMenu(Survey.Point p) {
        String[] items = {T.t("Rename"), T.t("Measure again here"), T.t("Delete")};
        new AlertDialog.Builder(this)
                .setTitle(p.name)
                .setItems(items, (d, which) -> {
                    if (which == 0) {
                        EditText e = Ui.field(this, p.name);
                        e.setText(p.name);
                        LinearLayout box = new LinearLayout(this);
                        box.setOrientation(LinearLayout.VERTICAL);
                        box.setPadding(dp(20), dp(8), dp(20), 0);
                        box.addView(e);
                        if (p.sent) box.addView(Ui.text(this, T.t("Renaming only changes the name on this phone."), 12, Ui.DIM), below(dp(8)));
                        new AlertDialog.Builder(this)
                                .setTitle(T.t("Rename"))
                                .setView(box)
                                .setPositiveButton(T.t("OK"), (x, w) -> {
                                    String n = e.getText().toString().trim();
                                    if (!n.isEmpty()) p.name = n;
                                    save();
                                    refreshPoints();
                                })
                                .setNegativeButton(T.t("Cancel"), null)
                                .show();
                    } else if (which == 1) {
                        if (p.x != null && !plan.walls.isEmpty()) {
                            plan.floor = p.floor;
                            if (planJson != null) readPlan(planJson);
                            plan.pick(p.x.floatValue(), p.y.floatValue());
                        }
                        setName(p.name);
                        nameTyped = true;
                        again = p;
                        measureStatus.setText(T.t("Spot picked — press Measure here."));
                        measureStatus.setTextColor(Ui.ACCENT);
                        ui.post(() -> scroll.smoothScrollTo(0, measureCard.getTop()));
                    } else {
                        new AlertDialog.Builder(this)
                                .setMessage(T.f("Delete '%s'? If NetSketch already has it, it stays there.", p.name))
                                .setPositiveButton(T.t("Delete"), (x, w) -> {
                                    survey.points.remove(p);
                                    save();
                                    resultsCard.setVisibility(View.GONE);
                                    refreshPoints();
                                })
                                .setNegativeButton(T.t("Keep"), null)
                                .show();
                    }
                })
                .show();
    }

    /** What one point shows: speed, advice, the channel picture, and every network, strongest first. */
    private void showResults(Survey.Point p) {
        resultsCard.setVisibility(View.VISIBLE);
        results.removeAllViews();
        resultsTitle.setText(T.f("Networks at %s · %d", p.name, p.networks.size()).toUpperCase(Locale.getDefault()));
        if (p.speed != null) {
            String text = p.speed.upMbps > 0
                    ? T.f("Real speed: %.0f down, %.0f up Mbit/s · %.0f ms (±%.0f ms)", p.speed.downMbps, p.speed.upMbps, p.speed.latencyMs, p.speed.jitterMs)
                    : T.f("Real speed: %.0f Mbit/s · %.0f ms delay", p.speed.downMbps, p.speed.latencyMs);
            TextView sp = Ui.title(this, text, 15);
            sp.setTextColor(Ui.ACCENT);
            sp.setPadding(0, 0, 0, dp(8));
            results.addView(sp);
        }
        List<String> advice = Survey.advice(p, mine);
        if (!advice.isEmpty()) {
            results.addView(Ui.heading(this, T.t("What this spot shows")));
            boolean good = advice.size() == 1 && advice.get(0).equals(T.t("Good spot: strong signal and no crowded channel."));
            for (String a : advice) {
                TextView t = Ui.text(this, "• " + a, 14, good ? Ui.GREEN : Ui.TEXT);
                t.setPadding(0, 0, 0, dp(6));
                results.addView(t);
            }
        }
        // Channel picture, one band at a time.
        Set<String> bands = new LinkedHashSet<>();
        for (String b : new String[] {"2.4 GHz", "5 GHz", "6 GHz"}) {
            for (Survey.Seen s : p.networks) if (Survey.bandOf(s.freq).equals(b)) bands.add(b);
        }
        if (!bands.isEmpty()) {
            TextView h = Ui.heading(this, T.t("Channels"));
            h.setPadding(0, dp(8), 0, dp(6));
            results.addView(h);
            Ui.Channels chart = new Ui.Channels(this);
            chart.nets.addAll(p.networks);
            for (Survey.Seen s : p.networks) if (Survey.isOwn(p, s, mine)) chart.own.add(s.bssid);
            chart.band = bands.iterator().next();
            LinearLayout chips = new LinearLayout(this);
            List<TextView> all = new ArrayList<>();
            for (String b : bands) {
                TextView chip = Ui.chip(this, b, Ui.ACCENT, b.equals(chart.band));
                LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
                lp.rightMargin = dp(8);
                chips.addView(chip, lp);
                all.add(chip);
                chip.setOnClickListener(v -> {
                    chart.band = b;
                    for (TextView o : all) {
                        boolean on = o == chip;
                        o.setTextColor(on ? Ui.BG : Ui.ACCENT);
                        o.setBackground(Ui.round(this, on ? Ui.ACCENT : android.graphics.Color.TRANSPARENT, 20, on ? 0 : Ui.ACCENT));
                    }
                    chart.invalidate();
                });
            }
            results.addView(chips);
            LinearLayout.LayoutParams clp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(150));
            clp.topMargin = dp(6);
            results.addView(chart, clp);
            TextView lh = Ui.heading(this, T.t("All networks"));
            lh.setPadding(0, dp(10), 0, dp(4));
            results.addView(lh);
        }
        LinearLayout list = new LinearLayout(this);
        list.setOrientation(LinearLayout.VERTICAL);
        results.addView(list);
        networkRows(p, list, SHORT_LIST);
        if (p.networks.size() > SHORT_LIST) {
            Button more = Ui.button(this, T.f("Show all %d", p.networks.size()), false);
            more.setOnClickListener(v -> {
                list.removeAllViews();
                networkRows(p, list, Integer.MAX_VALUE);
                more.setVisibility(View.GONE);
            });
            results.addView(more, below(dp(6)));
        }
    }

    private void networkRows(Survey.Point p, LinearLayout list, int max) {
        int n = 0;
        for (Survey.Seen s : p.networks) {
            if (n++ >= max) break;
            LinearLayout row = new LinearLayout(this);
            row.setGravity(Gravity.CENTER_VERTICAL);
            row.setPadding(0, dp(6), 0, dp(6));
            Ui.Bars bars = new Ui.Bars(this);
            bars.set(s.rssi);
            row.addView(bars);
            LinearLayout t = new LinearLayout(this);
            t.setOrientation(LinearLayout.VERTICAL);
            t.setPadding(dp(12), 0, dp(8), 0);
            boolean you = p.connected != null && s.bssid.equalsIgnoreCase(p.connected.bssid);
            boolean own = Survey.isOwn(p, s, mine);
            TextView name = Ui.title(this, (s.ssid.isEmpty() ? T.t("(hidden)") : s.ssid) + (you ? T.t("  · you") : own ? T.t("  · yours") : ""), 15);
            if (own) name.setTextColor(Ui.ACCENT);
            t.addView(name);
            String busy = s.utilization == null ? "" : T.f(" · %.0f%% busy", s.utilization * 100);
            String devs = s.stations == null ? "" : T.f(" · %d devices", s.stations);
            String security = s.security.equals("Open") ? T.t("Open (no password)") : s.security;
            t.addView(Ui.text(this, T.f("%s ch %d · %d MHz · %s", Survey.bandOf(s.freq), s.channel, s.width, security) + (s.standard.isEmpty() ? "" : " · " + s.standard) + busy + devs, 12, Ui.DIM));
            row.addView(t, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            TextView dbm = Ui.title(this, String.format(Locale.ROOT, "%.0f", s.rssi), 16);
            dbm.setTextColor(Ui.quality(s.rssi));
            row.addView(dbm);
            list.addView(row);
        }
    }

    // ---------- link to NetSketch ----------

    /** Finds NetSketch on the Wi-Fi (or takes a typed address), then asks for the code. */
    private void linkDialog() {
        LinearLayout box = new LinearLayout(this);
        box.setOrientation(LinearLayout.VERTICAL);
        box.setPadding(dp(20), dp(8), dp(20), 0);
        TextView info = Ui.text(this, T.t("On the computer: NetSketch → overview → Real measurements → Link a phone. Looking for it on this Wi-Fi…"), 14, Ui.DIM);
        box.addView(info);
        EditText addr = Ui.field(this, T.t("Address shown in NetSketch (if not found)"));
        addr.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        EditText code = Ui.field(this, T.t("4-digit code"));
        code.setInputType(InputType.TYPE_CLASS_NUMBER);
        box.addView(addr, below(dp(10)));
        box.addView(code, below(dp(10)));
        if (link != null) {
            addr.setText(link.host + ":" + link.port);
            code.setText(link.code);
        }
        AlertDialog d = new AlertDialog.Builder(this)
                .setTitle(T.t("Link to NetSketch"))
                .setView(box)
                .setPositiveButton(T.t("Link"), null)
                .setNeutralButton(link != null ? T.t("Unlink") : T.t("Cancel"), (x, w) -> {
                    if (link != null) {
                        link = null;
                        linkOk = false;
                        fetchFails = 0;
                        planJson = null;
                        prefs.edit().remove("host").remove("port").remove("code").remove("title").apply();
                        plan.clear();
                        plan.unpick();
                        computerPins.clear();
                        storeyRow.removeAllViews();
                        showLink();
                        refreshPoints();
                    }
                })
                .create();
        d.setOnShowListener(x -> d.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(v -> {
            Link l = Link.parseAddress(addr.getText().toString());
            if (l == null) {
                info.setText(T.t("Type the address NetSketch shows, e.g. 192.168.1.23:47878."));
                info.setTextColor(Ui.AMBER);
                return;
            }
            String typed = code.getText().toString().trim();
            if (!typed.matches("[0-9]{4}")) {
                info.setText(T.t("Enter the 4-digit code NetSketch shows."));
                info.setTextColor(Ui.AMBER);
                code.requestFocus();
                return;
            }
            String title = addr.getTag() instanceof String ? (String) addr.getTag() : "";
            link = new Link(l.host, l.port, typed, title);
            fetchFails = 0;
            d.dismiss();
            fetchPlan(true);
        }));
        d.show();
        // Look for NetSketch in the background and fill in the address.
        new Thread(() -> {
            Link found;
            try {
                found = Link.discover(2500);
            } catch (Exception e) {
                found = null;
            }
            final Link f = found;
            ui.post(() -> {
                if (!d.isShowing()) return;
                if (f != null) {
                    addr.setText(f.host + ":" + f.port);
                    addr.setTag(f.title);
                    info.setText(f.title.isEmpty() ? T.t("Found NetSketch. Enter the code it shows.") : T.f("Found NetSketch with '%s'. Enter the code it shows.", f.title));
                    info.setTextColor(Ui.ACCENT);
                    code.requestFocus();
                } else if (addr.getText().length() == 0) {
                    info.setText(T.t("Didn't find it automatically (some routers block that). Type the address NetSketch shows."));
                    info.setTextColor(Ui.AMBER);
                }
            });
        }).start();
    }

    /** Downloads the floor plan from NetSketch; then sends any points it doesn't have yet. */
    private void fetchPlan(boolean loud) {
        final Link l = link;
        if (l == null || l.code.isEmpty()) return;
        showLink();
        new Thread(() -> {
            try {
                String json = l.plan();
                ui.post(() -> {
                    if (link != l) return;
                    linkOk = true;
                    fetchFails = 0;
                    readPlan(json);
                    prefs.edit().putString("host", l.host).putInt("port", l.port).putString("code", l.code).putString("title", l.title).apply();
                    if (loud) Toast.makeText(this, T.t("Linked — tap where you're standing."), Toast.LENGTH_SHORT).show();
                    showLink();
                    sendUnsent(false);
                });
            } catch (Link.WrongCode e) {
                ui.post(() -> {
                    if (link != l) return;
                    link = l.withCode("");
                    linkOk = false;
                    prefs.edit().remove("code").apply();
                    Toast.makeText(this, T.t("NetSketch says the code is wrong — check the code it shows."), Toast.LENGTH_LONG).show();
                    showLink();
                });
            } catch (Exception e) {
                ui.post(() -> {
                    if (link != l) return;
                    linkOk = false;
                    if (loud) Toast.makeText(this, T.f("Couldn't reach NetSketch (%s). Is the computer on the same Wi-Fi and the link switched on?", why(e)), Toast.LENGTH_LONG).show();
                    // Every third miss in a row: the computer may have got a new address from the router.
                    if (++fetchFails % 3 == 0) rediscover(l);
                    showLink();
                });
            }
        }).start();
    }

    /** Looks for NetSketch on the Wi-Fi again and carries on with the same code at the address that answers. */
    private void rediscover(Link l) {
        if (rediscovering) return;
        rediscovering = true;
        new Thread(() -> {
            Link found;
            try {
                found = Link.discover(2500);
            } catch (Exception e) {
                found = null;
            }
            final Link f = found;
            ui.post(() -> {
                rediscovering = false;
                if (link != l || f == null) return;
                link = new Link(f.host, f.port, l.code, f.title.isEmpty() ? l.title : f.title);
                fetchPlan(false);
            });
        }).start();
    }

    /** Reads the plan NetSketch sent: storeys, walls, rooms, devices, your network names, earlier pins. */
    private void readPlan(String json) {
        try {
            JSONObject o = new JSONObject(json);
            planJson = json;
            plan.clear();
            JSONArray walls = o.optJSONArray("walls");
            for (int i = 0; walls != null && i < walls.length(); i++) {
                JSONObject w = walls.getJSONObject(i);
                PlanView.Wall x = new PlanView.Wall();
                x.floor = w.optInt("floor");
                x.ax = (float) w.getJSONArray("a").getDouble(0);
                x.ay = (float) w.getJSONArray("a").getDouble(1);
                x.bx = (float) w.getJSONArray("b").getDouble(0);
                x.by = (float) w.getJSONArray("b").getDouble(1);
                x.kind = w.optString("kind");
                plan.walls.add(x);
            }
            JSONArray labels = o.optJSONArray("labels");
            for (int i = 0; labels != null && i < labels.length(); i++) {
                JSONObject l = labels.getJSONObject(i);
                PlanView.Label x = new PlanView.Label();
                x.floor = l.optInt("floor");
                x.x = (float) l.getJSONArray("at").getDouble(0);
                x.y = (float) l.getJSONArray("at").getDouble(1);
                x.text = l.optString("text");
                plan.labels.add(x);
            }
            JSONArray devs = o.optJSONArray("devices");
            for (int i = 0; devs != null && i < devs.length(); i++) {
                JSONObject d = devs.getJSONObject(i);
                if ("Internet".equals(d.optString("kind"))) continue;
                PlanView.Dev x = new PlanView.Dev();
                x.floor = d.optInt("floor");
                x.x = (float) d.getJSONArray("at").getDouble(0);
                x.y = (float) d.getJSONArray("at").getDouble(1);
                x.name = d.optString("name");
                x.radio = d.optBoolean("radio");
                plan.devices.add(x);
            }
            computerPins.clear();
            JSONArray pts = o.optJSONArray("points");
            for (int i = 0; pts != null && i < pts.length(); i++) {
                JSONObject q = pts.getJSONObject(i);
                PlanView.Pin x = new PlanView.Pin();
                x.floor = q.optInt("floor");
                x.x = (float) q.getJSONArray("at").getDouble(0);
                x.y = (float) q.getJSONArray("at").getDouble(1);
                x.name = q.optString("name");
                x.fromComputer = true;
                computerPins.add(x);
            }
            JSONArray nets = o.optJSONArray("networks");
            if (nets != null) {
                mine.clear();
                for (int i = 0; i < nets.length(); i++) mine.add(nets.getString(i));
                prefs.edit().putStringSet("networks", new HashSet<>(mine)).apply();
            }
            // Storey chips.
            storeyRow.removeAllViews();
            JSONArray storeys = o.optJSONArray("storeys");
            List<Integer> floors = new ArrayList<>();
            for (int i = 0; storeys != null && i < storeys.length(); i++) {
                JSONObject s = storeys.getJSONObject(i);
                final int f = s.optInt("floor");
                floors.add(f);
                TextView chip = Ui.chip(this, s.optString("name"), Ui.ACCENT, f == plan.floor);
                LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
                lp.rightMargin = dp(8);
                chip.setOnClickListener(v -> {
                    if (plan.floor == f) return;
                    plan.floor = f;
                    plan.unpick();
                    again = null;
                    readPlan(json);
                });
                storeyRow.addView(chip, lp);
            }
            if (!floors.isEmpty() && !floors.contains(plan.floor)) {
                plan.floor = floors.get(0);
                plan.unpick();
                readPlan(json);
                return;
            }
            if (plan.selX == null) planHint.setText(storeys != null && storeys.length() > 1 ? T.t("Pick the storey, then tap where you're standing.") : T.t("Tap where you're standing."));
            refreshPoints();
        } catch (Exception e) {
            Toast.makeText(this, T.f("The floor plan couldn't be read: %s", why(e)), Toast.LENGTH_LONG).show();
        }
    }

    /** An error's own words, or a translated stand-in when it has none. */
    private static String why(Exception e) {
        String m = e.getMessage();
        return m == null || m.isEmpty() ? T.t("unknown error") : m;
    }

    /** Sends every point NetSketch doesn't have yet, one by one. */
    private void sendUnsent(boolean loud) {
        final Link l = link;
        if (l == null || l.code.isEmpty()) return;
        final List<Survey.Point> todo = new ArrayList<>();
        for (Survey.Point p : survey.points) if (!p.sent) todo.add(p);
        if (todo.isEmpty()) {
            if (loud) Toast.makeText(this, T.t("NetSketch already has every point."), Toast.LENGTH_SHORT).show();
            return;
        }
        // Already sending: that run takes these points too, or the next one does.
        if (!sending.compareAndSet(false, true)) return;
        new Thread(() -> {
            List<Survey.Point> ok = new ArrayList<>();
            String err = null;
            for (Survey.Point p : todo) {
                try {
                    l.send(survey.toJson(p));
                    ok.add(p);
                } catch (Exception e) {
                    err = why(e);
                    break;
                }
            }
            final int n = ok.size();
            final String why = err;
            ui.post(() -> {
                for (Survey.Point p : ok) p.sent = true;
                sending.set(false);
                save();
                refreshPoints();
                if (why != null) Toast.makeText(this, T.f("Kept on the phone for now: %s", why), Toast.LENGTH_LONG).show();
                else if (loud || n > 0) Toast.makeText(this, n == 1 ? T.t("Sent to NetSketch.") : T.f("Sent %d points to NetSketch.", n), Toast.LENGTH_SHORT).show();
            });
        }).start();
    }

    private void sendOrShare() {
        if (link != null && !link.code.isEmpty()) sendUnsent(true);
        else saveAndShare();
    }

    // ---------- measuring ----------

    /** What seeing nearby networks needs and isn't allowed yet. Android 12+ only asks for precise location together with coarse. */
    private List<String> missingPermissions() {
        List<String> need = new ArrayList<>();
        need.add(Manifest.permission.ACCESS_FINE_LOCATION);
        need.add(Manifest.permission.ACCESS_COARSE_LOCATION);
        if (Build.VERSION.SDK_INT >= 33) need.add(Manifest.permission.NEARBY_WIFI_DEVICES);
        List<String> missing = new ArrayList<>();
        for (String n : need) if (checkSelfPermission(n) != PackageManager.PERMISSION_GRANTED) missing.add(n);
        return missing;
    }

    /** Asks for what seeing nearby networks needs; true if it's all there already. Code 1 = then measure. */
    private boolean askPermissions(int code) {
        List<String> missing = missingPermissions();
        if (missing.isEmpty()) return true;
        requestPermissions(missing.toArray(new String[0]), code);
        return false;
    }

    /** Measure (or, quick, take a quick signal point) once everything it needs is there. Permission code 3 = then a quick point. */
    private void startMeasuring(boolean quick) {
        if (measuring || pinging) return;
        if (!askPermissions(quick ? 3 : 1)) return;
        LocationManager lm = getSystemService(LocationManager.class);
        if (lm != null && !lm.isLocationEnabled()) {
            new AlertDialog.Builder(this)
                    .setMessage(T.t("Android only shows the Wi-Fi networks around you while Location is on. Turn it on, then come back."))
                    .setPositiveButton(T.t("Open settings"), (d, w) -> startActivity(new Intent(Settings.ACTION_LOCATION_SOURCE_SETTINGS)))
                    .setNegativeButton(T.t("Cancel"), null)
                    .show();
            return;
        }
        if (!wifi.isWifiEnabled()) {
            Toast.makeText(this, T.t("Turn Wi-Fi on first."), Toast.LENGTH_LONG).show();
            return;
        }
        if (quick && connected() == null) {
            Toast.makeText(this, T.t("Connect to your Wi-Fi first: a quick point measures the network you're on."), Toast.LENGTH_LONG).show();
            return;
        }
        if (!plan.walls.isEmpty() && plan.selX == null) {
            new AlertDialog.Builder(this)
                    .setMessage(T.t("Tap where you're standing on the plan first — then the pin lands in the right place in NetSketch."))
                    .setPositiveButton(T.t("OK"), null)
                    .setNegativeButton(T.t("Measure anyway"), (d, w) -> begin(quick))
                    .show();
            return;
        }
        begin(quick);
    }

    /** Starts a point: three scans of everything heard (or, quick, a few seconds of the connection's own readings). */
    private void begin(boolean quickPoint) {
        if (measuring) return;
        measuring = true;
        quick = quickPoint;
        limited = false;
        failedStarts = 0;
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        measureButton.setText(T.t("Cancel"));
        for (Button b : new Button[] {quickButton, pingButton}) {
            b.setEnabled(false);
            b.setAlpha(0.5f);
        }
        current = new Survey.Point();
        String name = nameField.getText().toString().trim();
        current.name = name.isEmpty() ? T.f("Spot %d", survey.points.size() + 1) : name;
        if (plan.walls.isEmpty()) {
            try {
                current.floor = Math.max(-3, Math.min(20, Integer.parseInt(floorField.getText().toString().trim().replace('−', '-'))));
            } catch (NumberFormatException e) {
                current.floor = 0;
            }
        } else {
            current.floor = plan.floor;
            if (plan.selX != null) {
                current.x = (double) plan.selX;
                current.y = (double) plan.selY;
            }
        }
        if (again != null) {
            // Measured again: the same place as the point it replaces.
            current.floor = again.floor;
            current.x = again.x;
            current.y = again.y;
        }
        current.time = Instant.now().toString();
        current.connected = connected();
        heard = new LinkedHashMap<>();
        round = 0;
        roundStart = 0;
        scanPending = false;
        if (quick) {
            quickReadings = 0;
            quickRssi = 0;
            quickLink = 0;
            measureStatus.setText(T.f("Reading your Wi-Fi… %d s. Stand still.", 3));
            measureStatus.setTextColor(Ui.DIM);
            ui.post(quickTick);
            return;
        }
        receiver = new BroadcastReceiver() {
            @Override
            public void onReceive(Context c, Intent i) {
                if (scanPending) collectRound();
            }
        };
        IntentFilter f = new IntentFilter(WifiManager.SCAN_RESULTS_AVAILABLE_ACTION);
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(receiver, f, Context.RECEIVER_NOT_EXPORTED);
        else registerReceiver(receiver, f);
        scanRound();
    }

    @Override
    public void onRequestPermissionsResult(int code, String[] perms, int[] results) {
        if (results.length == 0) return; // the question was interrupted
        if (!missingPermissions().isEmpty()) {
            boolean coarseOnly = checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED
                    && checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED;
            measureStatus.setText(coarseOnly ? T.t("Seeing nearby networks needs your precise location: choose \"Precise\" when Android asks.")
                    : T.t("Without permission to see nearby networks, the phone can't measure anything."));
            measureStatus.setTextColor(Ui.AMBER);
            // After "Don't ask again" Android answers no by itself: only the app settings can change that.
            boolean never = false;
            for (String n : missingPermissions()) if (!shouldShowRequestPermissionRationale(n)) never = true;
            settingsButton.setVisibility(never ? View.VISIBLE : View.GONE);
            return;
        }
        settingsButton.setVisibility(View.GONE);
        showLive();
        if (code == 1 || code == 3) startMeasuring(code == 3);
    }

    /** How long until Android allows this app another scan (0 = now). */
    private long scanWaitMs() {
        if (Build.VERSION.SDK_INT >= 30 && !wifi.isScanThrottleEnabled()) return 0;
        long now = SystemClock.elapsedRealtime();
        scanTimes.removeIf(t -> now - t > SCAN_WINDOW_MS);
        if (scanTimes.size() < SCAN_BUDGET) return 0;
        return SCAN_WINDOW_MS - (now - scanTimes.get(0)) + 500;
    }

    private void waitForScan(long ms) {
        limited = true;
        throttleTip.setVisibility(View.VISIBLE);
        waitUntil = SystemClock.elapsedRealtime() + ms;
        ui.removeCallbacks(countdown);
        ui.post(countdown);
    }

    @SuppressWarnings("deprecation")
    private void scanRound() {
        if (!measuring) return;
        long wait = scanWaitMs();
        if (wait > 0) {
            // One round is enough for a point; only the first one is worth waiting for.
            if (round > 0) {
                limited = true;
                finishScans();
            } else {
                waitForScan(wait);
            }
            return;
        }
        measureStatus.setText(T.f("Listening… %d of %d", round + 1, ROUNDS) + " " + T.t("Stand still."));
        measureStatus.setTextColor(Ui.DIM);
        roundStart = SystemClock.elapsedRealtime();
        if (wifi.startScan()) {
            scanTimes.add(roundStart);
            scanPending = true;
            ui.postDelayed(roundTimeout, 12000);
        } else if (round > 0) {
            limited = true;
            finishScans();
        } else if (++failedStarts < 3) {
            waitForScan(10_000);
        } else {
            // Android keeps refusing: use what the phone heard in the last half minute.
            roundStart -= 30_000;
            ui.postDelayed(roundTimeout, 400);
        }
    }

    @SuppressWarnings("deprecation")
    private void collectRound() {
        if (!measuring) return;
        scanPending = false;
        ui.removeCallbacks(roundTimeout);
        // Only results from this round's scan: older ones may come from where the person stood before.
        long sinceUs = (roundStart - 2000) * 1000;
        for (ScanResult r : wifi.getScanResults()) {
            if (r.timestamp > 0 && r.timestamp < sinceUs) continue;
            Survey.Seen s = heard.get(r.BSSID);
            if (s == null) {
                s = seenOf(r);
                heard.put(r.BSSID, s);
            }
            s.sum += r.level;
            s.count++;
        }
        round++;
        if (round < ROUNDS) ui.postDelayed(nextRound, 2500);
        else finishScans();
    }

    /** What a scan says about one network (all but its strength, which is averaged over the rounds). */
    private static Survey.Seen seenOf(ScanResult r) {
        Survey.Seen s = new Survey.Seen();
        s.ssid = r.SSID == null ? "" : r.SSID;
        s.bssid = r.BSSID;
        s.freq = r.frequency;
        s.channel = Survey.channelOf(r.frequency);
        s.width = Survey.widthOf(r.channelWidth);
        s.security = Survey.securityOf(r.capabilities);
        if (Build.VERSION.SDK_INT >= 30) {
            s.standard = Survey.standardOf(r.getWifiStandard());
            for (ScanResult.InformationElement ie : r.getInformationElements()) {
                if (ie.getId() == 11) {
                    ByteBuffer b = ie.getBytes();
                    byte[] a = new byte[b.remaining()];
                    b.get(a);
                    double[] load = Survey.bssLoad(a);
                    if (load != null) {
                        s.stations = (int) load[0];
                        s.utilization = load[1];
                    }
                }
            }
        }
        return s;
    }

    /**
     * A quick point is the phone's own connection only, as one network heard (so NetSketch reads it like
     * any point). Width, security and standard come from Android's last scan when it has one; no new scan.
     */
    @SuppressWarnings("deprecation")
    private void finishQuick() {
        stopListening();
        Survey.Connected c = current.connected;
        if (c != null && quickReadings > 0) {
            c.rssi = quickRssi / quickReadings;
            c.linkMbps = quickLink / quickReadings;
            Survey.Seen s = null;
            for (ScanResult r : wifi.getScanResults()) if (c.bssid.equalsIgnoreCase(r.BSSID)) s = seenOf(r);
            if (s == null) {
                s = new Survey.Seen();
                s.ssid = c.ssid;
                s.bssid = c.bssid;
                s.freq = c.freq;
                s.channel = Survey.channelOf(c.freq);
                WifiInfo i = wifi.getConnectionInfo();
                if (Build.VERSION.SDK_INT >= 30 && i != null) s.standard = Survey.standardOf(i.getWifiStandard());
            }
            s.rssi = c.rssi;
            current.networks.add(s);
        }
        current.scans = 0;
        done(current);
    }

    private void stopListening() {
        if (receiver != null) {
            try {
                unregisterReceiver(receiver);
            } catch (IllegalArgumentException ignored) {
                // already gone
            }
            receiver = null;
        }
        scanPending = false;
        ui.removeCallbacks(roundTimeout);
        ui.removeCallbacks(nextRound);
        ui.removeCallbacks(countdown);
        ui.removeCallbacks(quickTick);
    }

    /** Back to "Measure here", with or without a new point. */
    private void endMeasuring() {
        stopListening();
        measuring = false;
        current = null;
        testing = null;
        getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        measureButton.setText(T.t("Measure here"));
        for (Button b : new Button[] {quickButton, pingButton}) {
            b.setEnabled(true);
            b.setAlpha(1f);
        }
    }

    /** Stops the scans or the speed test; no point is added. */
    private void cancelMeasuring() {
        endMeasuring();
        measureStatus.setText(T.t("Cancelled — no point added."));
        measureStatus.setTextColor(Ui.DIM);
    }

    private void finishScans() {
        stopListening();
        for (Survey.Seen s : heard.values()) {
            s.rssi = s.sum / Math.max(1, s.count);
            current.networks.add(s);
        }
        Collections.sort(current.networks, (a, b) -> Double.compare(b.rssi, a.rssi));
        current.scans = round;
        // The speed test only over Wi-Fi: on mobile data it would measure the wrong thing and cost data.
        boolean speed = speedSwitch.isChecked() && current.connected != null && !current.networks.isEmpty();
        final Network net = speed ? wifiNetwork() : null;
        if (speed && net == null) Toast.makeText(this, T.t("No Wi-Fi connection for the speed test, so it was skipped (it never uses mobile data)."), Toast.LENGTH_LONG).show();
        if (net != null) {
            measureStatus.setText(T.t("Testing the download…"));
            measureStatus.setTextColor(Ui.DIM);
            final Survey.Point p = current;
            testing = p;
            new Thread(() -> {
                Survey.Speed sp = speedTest(net, p);
                ui.post(() -> {
                    if (current != p) return; // cancelled
                    p.speed = sp;
                    if (sp == null) Toast.makeText(this, T.t("The speed test couldn't reach the internet."), Toast.LENGTH_LONG).show();
                    done(p);
                });
            }).start();
        } else {
            done(current);
        }
    }

    /** The phone's Wi-Fi network (not mobile data), or null. */
    @SuppressWarnings("deprecation")
    private Network wifiNetwork() {
        ConnectivityManager cm = getSystemService(ConnectivityManager.class);
        if (cm == null) return null;
        for (Network n : cm.getAllNetworks()) {
            NetworkCapabilities nc = cm.getNetworkCapabilities(n);
            if (nc != null && nc.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) return n;
        }
        return null;
    }

    /** The Wi-Fi's router (its default gateway), IPv4 if there is one. */
    private InetAddress gateway(Network net) {
        LinkProperties lp = getSystemService(ConnectivityManager.class).getLinkProperties(net);
        if (lp == null) return null;
        InetAddress other = null;
        for (RouteInfo r : lp.getRoutes()) {
            InetAddress g = r.getGateway();
            if (!r.isDefaultRoute() || g == null || g.isAnyLocalAddress()) continue;
            if (g instanceof Inet4Address) return g;
            if (other == null) other = g;
        }
        return other;
    }

    /** How quickly and steadily the router answers over Wi-Fi: 20 knocks, then average, jitter and loss. */
    private void pingTest() {
        if (measuring || pinging) return;
        final Network net = wifiNetwork();
        final InetAddress gw = net == null ? null : gateway(net);
        if (gw == null) {
            measureStatus.setText(net == null ? T.t("Connect to your Wi-Fi first.") : T.t("Couldn't find your router's address."));
            measureStatus.setTextColor(Ui.AMBER);
            return;
        }
        pinging = true;
        for (Button b : new Button[] {quickButton, pingButton}) {
            b.setEnabled(false);
            b.setAlpha(0.5f);
        }
        new Thread(() -> {
            List<Double> ms = new ArrayList<>();
            for (int i = 0; i < PINGS; i++) {
                final int n = i + 1;
                ui.post(() -> measureStatus.setText(T.f("Pinging your router… %d of %d", n, PINGS)));
                Double t = knock(net, gw);
                if (t != null) ms.add(t);
                SystemClock.sleep(150);
            }
            ui.post(() -> {
                pinging = false;
                for (Button b : new Button[] {quickButton, pingButton}) {
                    b.setEnabled(true);
                    b.setAlpha(1f);
                }
                if (ms.isEmpty()) {
                    measureStatus.setText(T.f("The router didn't answer any of %d tries.", PINGS));
                    measureStatus.setTextColor(Ui.RED);
                    return;
                }
                double sum = 0, jumps = 0;
                for (int i = 0; i < ms.size(); i++) {
                    sum += ms.get(i);
                    if (i > 0) jumps += Math.abs(ms.get(i) - ms.get(i - 1));
                }
                int lost = (PINGS - ms.size()) * 100 / PINGS;
                double avg = sum / ms.size(), jitter = ms.size() > 1 ? jumps / (ms.size() - 1) : 0;
                measureStatus.setText(T.f("Router ping: %.0f ms average, ±%.0f ms jitter, %d%% lost.", avg, jitter, lost));
                measureStatus.setTextColor(lost > 0 || avg > 30 ? Ui.AMBER : Ui.ACCENT);
            });
        }).start();
    }

    /**
     * One round trip to the router over Wi-Fi, in ms (null = no answer within a second). It opens a TCP
     * connection to the router's DNS port: an accepted or refused connection both answer, and it needs no root.
     * shortcut: TCP through the router's own stack is a little slower than an ICMP ping; fine for spotting a bad spot.
     */
    private static Double knock(Network net, InetAddress gw) {
        long t0 = System.nanoTime();
        try (Socket s = net.getSocketFactory().createSocket()) {
            s.connect(new InetSocketAddress(gw, 53), 1000);
            return (System.nanoTime() - t0) / 1e6;
        } catch (ConnectException e) {
            String m = e.getMessage();
            return m != null && m.contains("refused") ? (System.nanoTime() - t0) / 1e6 : null;
        } catch (Exception e) {
            return null;
        }
    }

    private void done(Survey.Point p) {
        endMeasuring();
        // Measured again: it takes the old point's place (sent as a new point if NetSketch had the old one).
        if (!survey.add(p, again)) {
            measureStatus.setText(T.t("No networks heard here. Try again."));
            measureStatus.setTextColor(Ui.AMBER);
            return;
        }
        again = null;
        save();
        measureButton.performHapticFeedback(Build.VERSION.SDK_INT >= 30 ? HapticFeedbackConstants.CONFIRM : HapticFeedbackConstants.LONG_PRESS);
        String msg = p.scans == 0 ? T.f("'%s': %.0f dBm. Walk to the next spot.", p.name, p.connected.rssi)
                : T.f("'%s': %d networks. Walk to the next spot.", p.name, p.networks.size());
        if (limited && p.scans < ROUNDS) msg += " " + T.f("Android limits scans: this spot used %d of %d rounds.", p.scans, ROUNDS);
        measureStatus.setText(msg);
        measureStatus.setTextColor(Ui.ACCENT);
        setName("");
        nameTyped = false;
        plan.unpick();
        showResults(p);
        refreshPoints();
        sendUnsent(false);
    }

    @SuppressWarnings("deprecation")
    private Survey.Connected connected() {
        WifiInfo i = wifi.getConnectionInfo();
        // Without permission Android hides the name and address, but the strength is still real.
        if (i == null || i.getRssi() <= -127 || i.getFrequency() <= 0) return null;
        Survey.Connected c = new Survey.Connected();
        String ssid = i.getSSID();
        c.ssid = ssid == null || ssid.equals(WifiManager.UNKNOWN_SSID) ? "" : ssid.replaceAll("^\"|\"$", "");
        c.bssid = i.getBSSID() == null ? "" : i.getBSSID();
        c.rssi = i.getRssi();
        c.linkMbps = i.getLinkSpeed();
        c.freq = i.getFrequency();
        return c;
    }

    /**
     * Delay and its variation (ten tiny requests: the best one, and the average jump between them), then
     * a 25 MB download and a 10 MB upload, each stopped after at most 12 s.
     */
    private Survey.Speed speedTest(Network net, Survey.Point p) {
        try {
            double best = Double.MAX_VALUE, jumps = 0, last = -1;
            for (int i = 0; i < 10; i++) {
                long t0 = System.nanoTime();
                fetch(net, p, "https://speed.cloudflare.com/__down?bytes=0", 0);
                double ms = (System.nanoTime() - t0) / 1e6;
                best = Math.min(best, ms);
                if (last >= 0) jumps += Math.abs(ms - last);
                last = ms;
            }
            long t0 = System.nanoTime();
            long bytes = fetch(net, p, "https://speed.cloudflare.com/__down?bytes=25000000", 12_000);
            double secs = (System.nanoTime() - t0) / 1e9;
            Survey.Speed s = new Survey.Speed();
            s.latencyMs = best;
            s.jitterMs = jumps / 9;
            s.downMbps = bytes * 8 / secs / 1e6;
            ui.post(() -> {
                if (testing == p) measureStatus.setText(T.t("Testing the upload…"));
            });
            try {
                s.upMbps = upload(net, p, "https://speed.cloudflare.com/__up", 10_000_000, 12_000);
            } catch (Exception e) {
                s.upMbps = 0; // the download still counts
            }
            return s;
        } catch (Exception e) {
            return null;
        }
    }

    /** Stops a speed test once it was cancelled (or another point is being measured). */
    private void stillFor(Survey.Point p) throws java.io.IOException {
        if (testing != p) throw new java.io.IOException("cancelled");
    }

    private long fetch(Network net, Survey.Point p, String url, long maxMs) throws Exception {
        stillFor(p);
        HttpURLConnection c = (HttpURLConnection) net.openConnection(new URL(url));
        c.setConnectTimeout(8000);
        c.setReadTimeout(8000);
        c.setRequestProperty("User-Agent", "NetSketch-Survey/0.1");
        long n = 0;
        long start = System.nanoTime();
        try (InputStream in = c.getInputStream()) {
            byte[] buf = new byte[64 * 1024];
            int r;
            while ((r = in.read(buf)) > 0) {
                stillFor(p);
                n += r;
                if (maxMs > 0 && (System.nanoTime() - start) / 1e6 > maxMs) break;
            }
        } finally {
            c.disconnect();
        }
        return n;
    }

    /** Sends up to maxBytes (or for maxMs) and returns the rate in Mbit/s, counted until the server answers. */
    private double upload(Network net, Survey.Point p, String url, long maxBytes, long maxMs) throws Exception {
        HttpURLConnection c = (HttpURLConnection) net.openConnection(new URL(url));
        c.setConnectTimeout(8000);
        c.setReadTimeout(15000);
        c.setRequestMethod("POST");
        c.setDoOutput(true);
        c.setChunkedStreamingMode(64 * 1024);
        c.setRequestProperty("Content-Type", "application/octet-stream");
        c.setRequestProperty("User-Agent", "NetSketch-Survey/0.1");
        byte[] buf = new byte[64 * 1024];
        long n = 0;
        long start = System.nanoTime();
        try {
            try (OutputStream out = c.getOutputStream()) {
                while (n < maxBytes && (System.nanoTime() - start) / 1e6 < maxMs) {
                    stillFor(p);
                    out.write(buf);
                    n += buf.length;
                }
            }
            int status = c.getResponseCode();
            if (status >= 400) throw new java.io.IOException("upload answered " + status);
        } finally {
            c.disconnect();
        }
        return n * 8 / ((System.nanoTime() - start) / 1e9) / 1e6;
    }

    // ---------- keeping and sharing ----------

    private File store() {
        return new File(getFilesDir(), "survey.json");
    }

    private void save() {
        survey.device = Build.MANUFACTURER + " " + Build.MODEL + " (Android " + Build.VERSION.RELEASE + ")";
        try {
            // Write next to it first, so a full disk or a crash never leaves half a file.
            File tmp = new File(getFilesDir(), "survey.json.new");
            Files.write(tmp.toPath(), survey.toJson().getBytes(StandardCharsets.UTF_8));
            Files.move(tmp.toPath(), store().toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING, java.nio.file.StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception e) {
            Toast.makeText(this, T.f("Couldn't keep the points: %s", why(e)), Toast.LENGTH_LONG).show();
        }
    }

    /** Reads the points kept from earlier sessions. */
    private void load() {
        try {
            if (!store().exists()) return;
            JSONObject o = new JSONObject(new String(Files.readAllBytes(store().toPath()), StandardCharsets.UTF_8));
            JSONArray pts = o.getJSONArray("points");
            for (int i = 0; i < pts.length(); i++) {
                JSONObject jp = pts.getJSONObject(i);
                Survey.Point p = new Survey.Point();
                p.name = jp.optString("name");
                p.floor = jp.optInt("floor");
                p.time = jp.optString("time");
                p.scans = jp.optInt("scans");
                p.sent = jp.optBoolean("sent");
                if (jp.has("x") && jp.has("y")) {
                    p.x = jp.optDouble("x");
                    p.y = jp.optDouble("y");
                }
                JSONObject jc = jp.optJSONObject("connected");
                if (jc != null) {
                    p.connected = new Survey.Connected();
                    p.connected.ssid = jc.optString("ssid");
                    p.connected.bssid = jc.optString("bssid");
                    p.connected.rssi = jc.optDouble("rssi");
                    p.connected.linkMbps = jc.optDouble("link_mbps");
                    p.connected.freq = jc.optInt("freq");
                }
                JSONObject js = jp.optJSONObject("speed");
                if (js != null) {
                    p.speed = new Survey.Speed();
                    p.speed.downMbps = js.optDouble("down_mbps");
                    p.speed.latencyMs = js.optDouble("latency_ms");
                    p.speed.upMbps = js.optDouble("up_mbps", 0);
                    p.speed.jitterMs = js.optDouble("jitter_ms", 0);
                }
                JSONArray nets = jp.getJSONArray("networks");
                for (int j = 0; j < nets.length(); j++) {
                    JSONObject jn = nets.getJSONObject(j);
                    Survey.Seen s = new Survey.Seen();
                    s.ssid = jn.optString("ssid");
                    s.bssid = jn.optString("bssid");
                    s.freq = jn.optInt("freq");
                    s.channel = jn.optInt("channel");
                    s.width = jn.optInt("width", 20);
                    s.rssi = jn.optDouble("rssi");
                    s.security = jn.optString("security");
                    s.standard = jn.optString("standard");
                    if (jn.has("stations")) s.stations = jn.optInt("stations");
                    if (jn.has("utilization")) s.utilization = jn.optDouble("utilization");
                    p.networks.add(s);
                }
                survey.points.add(p);
            }
        } catch (Exception e) {
            // Keep the damaged file for a look later instead of overwriting it with the next point.
            survey = new Survey();
            try {
                Files.move(store().toPath(), new File(getFilesDir(), "survey.json.bad").toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            } catch (Exception ignored) {
                // then the next save replaces it
            }
            Toast.makeText(this, T.t("The saved points couldn't be read. The damaged file was kept as survey.json.bad."), Toast.LENGTH_LONG).show();
        }
    }

    /** Writes the survey into Downloads and opens the share sheet (e-mail, cloud drive, nearby share…). */
    private void saveAndShare() {
        if (survey.points.isEmpty()) {
            Toast.makeText(this, T.t("Measure at least one spot first."), Toast.LENGTH_SHORT).show();
            return;
        }
        save();
        String name = "survey-" + new SimpleDateFormat("yyyyMMdd-HHmm", Locale.ROOT).format(new Date()) + ".netsurvey";
        ContentValues v = new ContentValues();
        v.put(MediaStore.MediaColumns.DISPLAY_NAME, name);
        v.put(MediaStore.MediaColumns.MIME_TYPE, "application/octet-stream");
        v.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS);
        Uri uri = getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, v);
        if (uri == null) {
            Toast.makeText(this, T.t("Couldn't save to Downloads."), Toast.LENGTH_LONG).show();
            return;
        }
        try (OutputStream out = getContentResolver().openOutputStream(uri)) {
            out.write(survey.toJson().getBytes(StandardCharsets.UTF_8));
        } catch (Exception e) {
            Toast.makeText(this, T.f("Couldn't save: %s", why(e)), Toast.LENGTH_LONG).show();
            return;
        }
        Toast.makeText(this, T.f("Saved %s in Downloads.", name), Toast.LENGTH_LONG).show();
        Intent send = new Intent(Intent.ACTION_SEND);
        send.setType("application/octet-stream");
        send.putExtra(Intent.EXTRA_STREAM, uri);
        send.putExtra(Intent.EXTRA_SUBJECT, T.t("Wi-Fi survey for NetSketch"));
        send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        startActivity(Intent.createChooser(send, T.t("Send the survey to your computer")));
    }
}
