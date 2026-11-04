package org.netsketch.survey;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.BroadcastReceiver;
import android.content.ContentValues;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.location.LocationManager;
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
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
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
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.text.SimpleDateFormat;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Date;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * NetSketch Survey. Link it to NetSketch on your computer (it finds it on the Wi-Fi), tap where you stand
 * on your floor plan, press Measure: every network heard there — and, if you like, the real speed — lands
 * in NetSketch as a pin in the right place. Without a computer nearby it keeps the points and shares them
 * as a file.
 */
public class MainActivity extends Activity {
    private static final int ROUNDS = 3;

    private final Handler ui = new Handler(Looper.getMainLooper());
    private Survey survey = new Survey();
    private WifiManager wifi;
    private SharedPreferences prefs;
    private Link link;

    // Screen parts that change.
    private TextView linkChip, liveDbm, liveWord, liveName, measureStatus, resultsTitle, pointsTitle, planHint;
    private Ui.Bars liveBars;
    private EditText nameField, floorField;
    private Switch speedSwitch;
    private Button measureButton, sendButton, clearButton, linkButton;
    private LinearLayout planCard, storeyRow, results, resultsCard, pointList, floorRow;
    private PlanView plan;
    private boolean nameTyped;

    // The measurement in progress.
    private Survey.Point current;
    private Map<String, Survey.Seen> heard;
    private int round;
    private boolean measuring;
    private BroadcastReceiver receiver;
    private final Runnable roundTimeout = this::collectRound;
    private final Runnable live = new Runnable() {
        @Override
        public void run() {
            showLive();
            ui.postDelayed(this, 1000);
        }
    };

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        requestWindowFeature(Window.FEATURE_NO_TITLE);
        getWindow().setStatusBarColor(Ui.BG);
        getWindow().setNavigationBarColor(Ui.BG);
        wifi = getApplicationContext().getSystemService(WifiManager.class);
        prefs = getSharedPreferences("link", MODE_PRIVATE);
        load();
        buildUi();
        askPermissions(2);
        refreshPoints();
        // Reconnect to the computer used last time, quietly.
        String host = prefs.getString("host", null);
        if (host != null) {
            link = new Link(host, prefs.getInt("port", Link.DEFAULT_PORT), prefs.getString("code", ""), prefs.getString("title", ""));
            fetchPlan(false);
        }
        showLink();
    }

    @Override
    protected void onResume() {
        super.onResume();
        ui.post(live);
    }

    @Override
    protected void onPause() {
        ui.removeCallbacks(live);
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        stopListening();
        super.onDestroy();
    }

    // ---------- screen ----------

    private int dp(float v) {
        return Ui.dp(this, v);
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
        TextView app = Ui.title(this, "NetSketch Survey", 22);
        names.addView(app);
        names.addView(Ui.text(this, "Your real Wi-Fi, spot by spot", 13, Ui.DIM));
        head.addView(names, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        linkChip = Ui.chip(this, "Link", Ui.DIM, false);
        linkChip.setSingleLine(true);
        linkChip.setEllipsize(android.text.TextUtils.TruncateAt.END);
        linkChip.setMaxWidth(dp(170));
        linkChip.setOnClickListener(v -> linkDialog());
        head.addView(linkChip);
        root.addView(head);

        // Live signal of the phone's own Wi-Fi.
        LinearLayout liveCard = Ui.card(this);
        liveCard.addView(Ui.heading(this, "Your Wi-Fi right here"));
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
        root.addView(liveCard);

        // Floor plan (from NetSketch) — tap where you stand.
        planCard = Ui.card(this);
        planCard.addView(Ui.heading(this, "Where are you?"));
        HorizontalScrollView storeyScroll = new HorizontalScrollView(this);
        storeyScroll.setHorizontalScrollBarEnabled(false);
        storeyRow = new LinearLayout(this);
        storeyScroll.addView(storeyRow);
        planCard.addView(storeyScroll);
        plan = new PlanView(this);
        LinearLayout.LayoutParams plp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(300));
        plp.topMargin = dp(8);
        planCard.addView(plan, plp);
        plan.onSpot = (x, y) -> {
            String room = plan.roomAt(x, y);
            if (room != null && !nameTyped) {
                nameField.setText(room);
                nameTyped = false;
            }
            planHint.setText(String.format(Locale.ROOT, "Spot picked: %.1f m, %.1f m — measure when you're standing there.", x, y));
        };
        planHint = Ui.text(this, "Link to NetSketch on your computer to tap your spot on the floor plan.", 13, Ui.DIM);
        planHint.setPadding(0, dp(8), 0, 0);
        planCard.addView(planHint);
        linkButton = Ui.button(this, "Link to NetSketch", false);
        linkButton.setOnClickListener(v -> linkDialog());
        LinearLayout.LayoutParams llp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        llp.topMargin = dp(10);
        planCard.addView(linkButton, llp);
        root.addView(planCard);

        // Name, storey, speed test, and the big button.
        LinearLayout m = Ui.card(this);
        m.addView(Ui.heading(this, "Measure"));
        nameField = Ui.field(this, "Name this spot, e.g. Sofa");
        nameField.setOnKeyListener((v, k, e) -> {
            nameTyped = true;
            return false;
        });
        m.addView(nameField);
        floorRow = new LinearLayout(this);
        floorRow.setGravity(Gravity.CENTER_VERTICAL);
        floorRow.setPadding(0, dp(10), 0, 0);
        floorRow.addView(Ui.text(this, "Storey  ", 15, Ui.TEXT));
        floorField = Ui.field(this, "0");
        floorField.setInputType(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_SIGNED);
        floorField.setText("0");
        floorField.setEms(3);
        floorRow.addView(floorField);
        floorRow.addView(Ui.text(this, "   0 = ground, −1 = basement", 13, Ui.DIM));
        m.addView(floorRow);
        speedSwitch = new Switch(this);
        speedSwitch.setText("Speed test too (uses 25 MB)");
        speedSwitch.setTextColor(Ui.TEXT);
        speedSwitch.setTextSize(15);
        speedSwitch.setPadding(0, dp(12), 0, dp(4));
        m.addView(speedSwitch);
        measureButton = Ui.button(this, "Measure here", true);
        measureButton.setTextSize(18);
        LinearLayout.LayoutParams blp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(60));
        blp.topMargin = dp(10);
        m.addView(measureButton, blp);
        measureButton.setOnClickListener(v -> startMeasuring());
        measureStatus = Ui.text(this, "Stand still for about 10 seconds while it listens.", 13, Ui.DIM);
        measureStatus.setPadding(0, dp(8), 0, 0);
        m.addView(measureStatus);
        root.addView(m);

        // What was heard at the last (or a chosen) point.
        resultsCard = Ui.card(this);
        resultsTitle = Ui.heading(this, "Networks here");
        resultsCard.addView(resultsTitle);
        results = new LinearLayout(this);
        results.setOrientation(LinearLayout.VERTICAL);
        resultsCard.addView(results);
        resultsCard.setVisibility(View.GONE);
        root.addView(resultsCard);

        // All points, and getting them to NetSketch.
        LinearLayout pc = Ui.card(this);
        pointsTitle = Ui.heading(this, "Points");
        pc.addView(pointsTitle);
        pointList = new LinearLayout(this);
        pointList.setOrientation(LinearLayout.VERTICAL);
        pc.addView(pointList);
        LinearLayout actions = new LinearLayout(this);
        actions.setPadding(0, dp(12), 0, 0);
        sendButton = Ui.button(this, "Send to NetSketch", false);
        sendButton.setOnClickListener(v -> sendOrShare());
        actions.addView(sendButton, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        actions.addView(Ui.gap(this, 0), new LinearLayout.LayoutParams(dp(8), 1));
        Button clear = Ui.button(this, "Start over", false);
        clearButton = clear;
        clear.setOnClickListener(v -> new AlertDialog.Builder(this)
                .setMessage("Delete all " + survey.points.size() + " points on this phone? Points already in NetSketch stay there.")
                .setPositiveButton("Delete", (d, w) -> {
                    survey.points.clear();
                    save();
                    refreshPoints();
                })
                .setNegativeButton("Keep", null)
                .show());
        actions.addView(clear);
        pc.addView(actions);
        root.addView(pc);

        TextView how = Ui.text(this, "Tip: measure where you actually use Wi-Fi (desk, sofa, bed), the far corners, and near the walls to your neighbours. "
                + "Networks you don't own can then be added as neighbours in NetSketch with one click, with the channels and busy time you measured.", 12, Ui.DIM);
        how.setPadding(dp(4), dp(16), dp(4), 0);
        root.addView(how);

        ScrollView scroll = new ScrollView(this);
        scroll.setBackgroundColor(Ui.BG);
        scroll.setFillViewport(true);
        scroll.setClipToPadding(false);
        // Android 15 draws apps under the status and navigation bars: leave room for them.
        scroll.setOnApplyWindowInsetsListener((v, in) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets b = in.getInsets(android.view.WindowInsets.Type.systemBars() | android.view.WindowInsets.Type.displayCutout());
                v.setPadding(b.left, b.top, b.right, b.bottom);
            } else {
                v.setPadding(in.getSystemWindowInsetLeft(), in.getSystemWindowInsetTop(), in.getSystemWindowInsetRight(), in.getSystemWindowInsetBottom());
            }
            return in;
        });
        scroll.addView(root);
        setContentView(scroll);
    }

    /** The live signal card, once a second. */
    private void showLive() {
        Survey.Connected c = connected();
        if (c == null) {
            liveDbm.setText("—");
            liveDbm.setTextColor(Ui.DIM);
            liveWord.setText("Not on Wi-Fi");
            liveName.setText("Connect to your Wi-Fi to see its strength here.");
            liveBars.set(-100);
            return;
        }
        liveDbm.setText(String.format(Locale.ROOT, "%.0f", c.rssi));
        liveDbm.setTextColor(Ui.quality(c.rssi));
        liveWord.setText("dBm · " + Ui.qualityWord(c.rssi));
        liveName.setText(String.format(Locale.ROOT, "%s · %s ch %d · %.0f Mbit/s link", c.ssid.isEmpty() ? "name hidden until you allow nearby networks" : c.ssid, Survey.bandOf(c.freq), Survey.channelOf(c.freq), c.linkMbps));
        liveBars.set(c.rssi);
    }

    private void showLink() {
        boolean on = link != null && !link.code.isEmpty() && !plan.walls.isEmpty();
        boolean known = link != null && !link.code.isEmpty();
        linkChip.setText(on ? "Linked · " + (link.title.isEmpty() ? link.host : link.title) : known ? "Linking…" : "Link");
        linkChip.setTextColor(on ? Ui.BG : known ? Ui.AMBER : Ui.ACCENT);
        linkChip.setBackground(Ui.round(this, on ? Ui.ACCENT : android.graphics.Color.TRANSPARENT, 20, on ? 0 : known ? Ui.AMBER : Ui.ACCENT));
        floorRow.setVisibility(plan.walls.isEmpty() ? View.VISIBLE : View.GONE);
        storeyRow.setVisibility(plan.walls.isEmpty() ? View.GONE : View.VISIBLE);
        plan.setVisibility(plan.walls.isEmpty() ? View.GONE : View.VISIBLE);
        sendButton.setText(known ? "Send to NetSketch" : "Save & share file");
        linkButton.setVisibility(known ? View.GONE : View.VISIBLE);
        if (!known) planHint.setText("Link to NetSketch on your computer to tap your spot on the floor plan.");
    }

    private void refreshPoints() {
        pointList.removeAllViews();
        int unsent = 0;
        for (Survey.Point p : survey.points) if (!p.sent) unsent++;
        pointsTitle.setText(("Points · " + survey.points.size() + (unsent > 0 && link != null ? " · " + unsent + " not sent yet" : "")).toUpperCase());
        if (survey.points.isEmpty()) {
            pointList.addView(Ui.text(this, "None yet. Measure your first spot above.", 14, Ui.DIM));
        }
        clearButton.setVisibility(survey.points.isEmpty() ? View.GONE : View.VISIBLE);
        sendButton.setEnabled(!survey.points.isEmpty());
        sendButton.setAlpha(survey.points.isEmpty() ? 0.5f : 1f);
        for (int i = survey.points.size() - 1; i >= 0; i--) {
            final Survey.Point p = survey.points.get(i);
            LinearLayout row = new LinearLayout(this);
            row.setGravity(Gravity.CENTER_VERTICAL);
            row.setPadding(0, dp(8), 0, dp(8));
            LinearLayout t = new LinearLayout(this);
            t.setOrientation(LinearLayout.VERTICAL);
            t.addView(Ui.title(this, p.name, 15));
            String where = p.x != null ? "placed on the plan" : "pin to place on the computer";
            String speed = p.speed == null ? "" : String.format(Locale.ROOT, " · %.0f Mbit/s", p.speed.downMbps);
            t.addView(Ui.text(this, p.networks.size() + " networks · " + where + speed, 12, Ui.DIM));
            t.setOnClickListener(v -> showResults(p));
            row.addView(t, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            row.addView(Ui.chip(this, p.sent ? "In NetSketch" : "On phone", p.sent ? Ui.GREEN : Ui.DIM, false));
            TextView del = Ui.text(this, "  ✕", 18, Ui.DIM);
            del.setPadding(dp(8), 0, 0, 0);
            del.setOnClickListener(v -> {
                survey.points.remove(p);
                save();
                refreshPoints();
            });
            row.addView(del);
            pointList.addView(row);
        }
        // Earlier points show on the plan as small amber dots.
        plan.pins.clear();
        for (Survey.Point p : survey.points) {
            if (p.x == null) continue;
            PlanView.Pin q = new PlanView.Pin();
            q.floor = p.floor;
            q.x = p.x.floatValue();
            q.y = p.y.floatValue();
            q.name = p.name;
            plan.pins.add(q);
        }
        plan.invalidate();
    }

    /** The networks heard at one point, strongest first. */
    private void showResults(Survey.Point p) {
        resultsCard.setVisibility(View.VISIBLE);
        results.removeAllViews();
        resultsTitle.setText(("Networks at " + p.name + " · " + p.networks.size()).toUpperCase());
        if (p.speed != null) {
            TextView sp = Ui.title(this, String.format(Locale.ROOT, "Real speed: %.0f Mbit/s · %.0f ms delay", p.speed.downMbps, p.speed.latencyMs), 15);
            sp.setTextColor(Ui.ACCENT);
            sp.setPadding(0, 0, 0, dp(8));
            results.addView(sp);
        }
        for (Survey.Seen s : p.networks) {
            LinearLayout row = new LinearLayout(this);
            row.setGravity(Gravity.CENTER_VERTICAL);
            row.setPadding(0, dp(6), 0, dp(6));
            Ui.Bars bars = new Ui.Bars(this);
            bars.set(s.rssi);
            row.addView(bars);
            LinearLayout t = new LinearLayout(this);
            t.setOrientation(LinearLayout.VERTICAL);
            t.setPadding(dp(12), 0, dp(8), 0);
            boolean mine = p.connected != null && s.bssid.equalsIgnoreCase(p.connected.bssid);
            t.addView(Ui.title(this, (s.ssid.isEmpty() ? "(hidden)" : s.ssid) + (mine ? "  · you" : ""), 15));
            String busy = s.utilization == null ? "" : String.format(Locale.ROOT, " · %.0f%% busy", s.utilization * 100);
            t.addView(Ui.text(this, Survey.bandOf(s.freq) + " ch " + s.channel + " · " + s.width + " MHz · " + s.security + (s.standard.isEmpty() ? "" : " · " + s.standard) + busy, 12, Ui.DIM));
            row.addView(t, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            TextView dbm = Ui.title(this, String.format(Locale.ROOT, "%.0f", s.rssi), 16);
            dbm.setTextColor(Ui.quality(s.rssi));
            row.addView(dbm);
            results.addView(row);
        }
    }

    // ---------- link to NetSketch ----------

    /** Finds NetSketch on the Wi-Fi (or takes a typed address), then asks for the code. */
    private void linkDialog() {
        LinearLayout box = new LinearLayout(this);
        box.setOrientation(LinearLayout.VERTICAL);
        box.setPadding(dp(20), dp(8), dp(20), 0);
        TextView info = Ui.text(this, "On the computer: NetSketch → overview → Real measurements → Link a phone. Looking for it on this Wi-Fi…", 14, Ui.DIM);
        box.addView(info);
        EditText addr = Ui.field(this, "Address shown in NetSketch (if not found)");
        addr.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        EditText code = Ui.field(this, "4-digit code");
        code.setInputType(InputType.TYPE_CLASS_NUMBER);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.topMargin = dp(10);
        box.addView(addr, lp);
        box.addView(code, new LinearLayout.LayoutParams(lp));
        if (link != null) {
            addr.setText(link.host + ":" + link.port);
            code.setText(link.code);
        }
        AlertDialog d = new AlertDialog.Builder(this)
                .setTitle("Link to NetSketch")
                .setView(box)
                .setPositiveButton("Link", null)
                .setNeutralButton(link != null ? "Unlink" : "Cancel", (x, w) -> {
                    if (link != null) {
                        link = null;
                        prefs.edit().clear().apply();
                        plan.clear();
                        plan.invalidate();
                        showLink();
                        refreshPoints();
                    }
                })
                .create();
        d.setOnShowListener(x -> d.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(v -> {
            Link l = Link.parseAddress(addr.getText().toString());
            if (l == null) {
                info.setText("Type the address NetSketch shows, e.g. 192.168.1.23:47878.");
                info.setTextColor(Ui.AMBER);
                return;
            }
            String title = addr.getTag() instanceof String ? (String) addr.getTag() : "";
            link = new Link(l.host, l.port, code.getText().toString().trim(), title);
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
                if (f != null) {
                    addr.setText(f.host + ":" + f.port);
                    addr.setTag(f.title);
                    info.setText("Found NetSketch" + (f.title.isEmpty() ? "" : " with '" + f.title + "'") + ". Enter the code it shows.");
                    info.setTextColor(Ui.ACCENT);
                    code.requestFocus();
                } else if (addr.getText().length() == 0) {
                    info.setText("Didn't find it automatically (some routers block that). Type the address NetSketch shows.");
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
                    readPlan(json);
                    prefs.edit().putString("host", l.host).putInt("port", l.port).putString("code", l.code).putString("title", l.title).apply();
                    if (loud) Toast.makeText(this, "Linked — tap where you're standing.", Toast.LENGTH_SHORT).show();
                    showLink();
                    sendUnsent(false);
                });
            } catch (Link.WrongCode e) {
                ui.post(() -> {
                    link = l.withCode("");
                    Toast.makeText(this, "NetSketch says the code is wrong — check the code it shows.", Toast.LENGTH_LONG).show();
                    showLink();
                });
            } catch (Exception e) {
                ui.post(() -> {
                    if (loud) Toast.makeText(this, "Couldn't reach NetSketch (" + e.getMessage() + "). Is the computer on the same Wi-Fi and the link switched on?", Toast.LENGTH_LONG).show();
                    showLink();
                });
            }
        }).start();
    }

    /** Reads the plan NetSketch sent: storeys, walls, rooms, devices, earlier pins. */
    private void readPlan(String json) {
        try {
            JSONObject o = new JSONObject(json);
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
                String kind = w.optString("kind");
                x.color = PlanView.wallColor(kind);
                x.thick = PlanView.wallThick(kind);
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
                    plan.floor = f;
                    plan.selX = null;
                    plan.selY = null;
                    readPlan(json);
                });
                storeyRow.addView(chip, lp);
            }
            if (!floors.isEmpty() && !floors.contains(plan.floor)) {
                plan.floor = floors.get(0);
                readPlan(json);
                return;
            }
            planHint.setText(storeys != null && storeys.length() > 1 ? "Pick the storey, then tap where you're standing." : "Tap where you're standing.");
            refreshPoints();
        } catch (Exception e) {
            Toast.makeText(this, "The floor plan couldn't be read: " + e.getMessage(), Toast.LENGTH_LONG).show();
        }
    }

    /** Sends every point NetSketch doesn't have yet, one by one. */
    private void sendUnsent(boolean loud) {
        final Link l = link;
        if (l == null || l.code.isEmpty()) return;
        final List<Survey.Point> todo = new ArrayList<>();
        for (Survey.Point p : survey.points) if (!p.sent) todo.add(p);
        if (todo.isEmpty()) {
            if (loud) Toast.makeText(this, "NetSketch already has every point.", Toast.LENGTH_SHORT).show();
            return;
        }
        new Thread(() -> {
            int ok = 0;
            String err = null;
            for (Survey.Point p : todo) {
                try {
                    l.send(survey.toJson(p));
                    p.sent = true;
                    ok++;
                } catch (Exception e) {
                    err = e.getMessage();
                    break;
                }
            }
            final int n = ok;
            final String why = err;
            ui.post(() -> {
                save();
                refreshPoints();
                if (why != null) Toast.makeText(this, "Kept on the phone for now: " + why, Toast.LENGTH_LONG).show();
                else if (loud || n > 0) Toast.makeText(this, n == 1 ? "Sent to NetSketch." : "Sent " + n + " points to NetSketch.", Toast.LENGTH_SHORT).show();
            });
        }).start();
    }

    private void sendOrShare() {
        if (link != null && !link.code.isEmpty()) sendUnsent(true);
        else saveAndShare();
    }

    // ---------- measuring ----------

    /** Asks for what seeing nearby networks needs; true if it's all there already. Code 1 = then measure. */
    private boolean askPermissions(int code) {
        List<String> need = new ArrayList<>();
        need.add(Manifest.permission.ACCESS_FINE_LOCATION);
        if (Build.VERSION.SDK_INT >= 33) need.add(Manifest.permission.NEARBY_WIFI_DEVICES);
        List<String> missing = new ArrayList<>();
        for (String n : need) if (checkSelfPermission(n) != PackageManager.PERMISSION_GRANTED) missing.add(n);
        if (missing.isEmpty()) return true;
        requestPermissions(missing.toArray(new String[0]), code);
        return false;
    }

    private void startMeasuring() {
        if (measuring) return;
        if (!askPermissions(1)) return;
        LocationManager lm = getSystemService(LocationManager.class);
        if (lm != null && !lm.isLocationEnabled()) {
            new AlertDialog.Builder(this)
                    .setMessage("Android only shows the Wi-Fi networks around you while Location is on. Turn it on, then come back.")
                    .setPositiveButton("Open settings", (d, w) -> startActivity(new Intent(Settings.ACTION_LOCATION_SOURCE_SETTINGS)))
                    .setNegativeButton("Cancel", null)
                    .show();
            return;
        }
        if (!wifi.isWifiEnabled()) {
            Toast.makeText(this, "Turn Wi-Fi on first.", Toast.LENGTH_LONG).show();
            return;
        }
        if (!plan.walls.isEmpty() && plan.selX == null) {
            new AlertDialog.Builder(this)
                    .setMessage("Tap where you're standing on the plan first — then the pin lands in the right place in NetSketch.")
                    .setPositiveButton("OK", null)
                    .setNegativeButton("Measure anyway", (d, w) -> begin())
                    .show();
            return;
        }
        begin();
    }

    private void begin() {
        measuring = true;
        measureButton.setEnabled(false);
        measureButton.setAlpha(0.6f);
        current = new Survey.Point();
        String name = nameField.getText().toString().trim();
        current.name = name.isEmpty() ? "Spot " + (survey.points.size() + 1) : name;
        if (plan.walls.isEmpty()) {
            try {
                current.floor = Integer.parseInt(floorField.getText().toString().trim().replace('−', '-'));
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
        current.time = Instant.now().toString();
        current.connected = connected();
        heard = new LinkedHashMap<>();
        round = 0;
        receiver = new BroadcastReceiver() {
            @Override
            public void onReceive(Context c, Intent i) {
                collectRound();
            }
        };
        IntentFilter f = new IntentFilter(WifiManager.SCAN_RESULTS_AVAILABLE_ACTION);
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(receiver, f, Context.RECEIVER_NOT_EXPORTED);
        else registerReceiver(receiver, f);
        scanRound();
    }

    @Override
    public void onRequestPermissionsResult(int code, String[] perms, int[] results) {
        for (int r : results) {
            if (r != PackageManager.PERMISSION_GRANTED) {
                measureStatus.setText("Without permission to see nearby networks, the phone can't measure anything.");
                measureStatus.setTextColor(Ui.AMBER);
                return;
            }
        }
        showLive();
        if (code == 1) startMeasuring();
    }

    @SuppressWarnings("deprecation")
    private void scanRound() {
        measureButton.setText("Listening… " + (round + 1) + " of " + ROUNDS);
        measureStatus.setText("Stand still.");
        measureStatus.setTextColor(Ui.DIM);
        if (!wifi.startScan()) {
            // Android limits apps to a few scans every two minutes: use what it heard last.
            measureStatus.setText("Android limits how often apps may scan — using the phone's latest results.");
            ui.postDelayed(this::collectRound, 400);
        } else {
            ui.postDelayed(roundTimeout, 12000);
        }
    }

    @SuppressWarnings("deprecation")
    private void collectRound() {
        if (!measuring) return;
        ui.removeCallbacks(roundTimeout);
        long nowUs = SystemClock.elapsedRealtime() * 1000;
        for (ScanResult r : wifi.getScanResults()) {
            // Skip results the phone remembers from more than a minute ago (somewhere else).
            if (r.timestamp > 0 && nowUs - r.timestamp > 60_000_000L) continue;
            Survey.Seen s = heard.get(r.BSSID);
            if (s == null) {
                s = new Survey.Seen();
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
                heard.put(r.BSSID, s);
            }
            s.sum += r.level;
            s.count++;
        }
        round++;
        if (round < ROUNDS) ui.postDelayed(this::scanRound, 2500);
        else finishScans();
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
        ui.removeCallbacks(roundTimeout);
    }

    private void finishScans() {
        stopListening();
        for (Survey.Seen s : heard.values()) {
            s.rssi = s.sum / Math.max(1, s.count);
            current.networks.add(s);
        }
        Collections.sort(current.networks, (a, b) -> Double.compare(b.rssi, a.rssi));
        current.scans = round;
        if (speedSwitch.isChecked() && current.connected != null) {
            measureButton.setText("Testing the speed…");
            final Survey.Point p = current;
            new Thread(() -> {
                Survey.Speed sp = speedTest();
                ui.post(() -> {
                    p.speed = sp;
                    if (sp == null) Toast.makeText(this, "The speed test couldn't reach the internet.", Toast.LENGTH_LONG).show();
                    done(p);
                });
            }).start();
        } else {
            done(current);
        }
    }

    private void done(Survey.Point p) {
        survey.points.add(p);
        save();
        measuring = false;
        measureButton.setEnabled(true);
        measureButton.setAlpha(1f);
        measureButton.setText("Measure here");
        measureStatus.setText(String.format(Locale.ROOT, "'%s': %d networks. Walk to the next spot.", p.name, p.networks.size()));
        measureStatus.setTextColor(Ui.ACCENT);
        nameField.setText("");
        nameTyped = false;
        plan.selX = null;
        plan.selY = null;
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

    /** Delay to a nearby server (best of five tiny requests), then a 25 MB download (at most 12 s). */
    private static Survey.Speed speedTest() {
        try {
            double best = Double.MAX_VALUE;
            for (int i = 0; i < 5; i++) {
                long t0 = System.nanoTime();
                fetch("https://speed.cloudflare.com/__down?bytes=0", 0);
                best = Math.min(best, (System.nanoTime() - t0) / 1e6);
            }
            long t0 = System.nanoTime();
            long bytes = fetch("https://speed.cloudflare.com/__down?bytes=25000000", 12_000);
            double secs = (System.nanoTime() - t0) / 1e9;
            Survey.Speed s = new Survey.Speed();
            s.latencyMs = best;
            s.downMbps = bytes * 8 / secs / 1e6;
            return s;
        } catch (Exception e) {
            return null;
        }
    }

    private static long fetch(String url, long maxMs) throws Exception {
        HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
        c.setConnectTimeout(8000);
        c.setReadTimeout(8000);
        c.setRequestProperty("User-Agent", "NetSketch-Survey/1.0");
        long n = 0;
        long start = System.nanoTime();
        try (InputStream in = c.getInputStream()) {
            byte[] buf = new byte[64 * 1024];
            int r;
            while ((r = in.read(buf)) > 0) {
                n += r;
                if (maxMs > 0 && (System.nanoTime() - start) / 1e6 > maxMs) break;
            }
        } finally {
            c.disconnect();
        }
        return n;
    }

    // ---------- keeping and sharing ----------

    private File store() {
        return new File(getFilesDir(), "survey.json");
    }

    private void save() {
        survey.device = Build.MANUFACTURER + " " + Build.MODEL + " (Android " + Build.VERSION.RELEASE + ")";
        try {
            Files.write(store().toPath(), survey.toJson().getBytes(StandardCharsets.UTF_8));
        } catch (Exception e) {
            Toast.makeText(this, "Couldn't keep the points: " + e.getMessage(), Toast.LENGTH_LONG).show();
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
            survey = new Survey();
        }
    }

    /** Writes the survey into Downloads and opens the share sheet (e-mail, cloud drive, nearby share…). */
    private void saveAndShare() {
        if (survey.points.isEmpty()) {
            Toast.makeText(this, "Measure at least one spot first.", Toast.LENGTH_SHORT).show();
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
            Toast.makeText(this, "Couldn't save to Downloads.", Toast.LENGTH_LONG).show();
            return;
        }
        try (OutputStream out = getContentResolver().openOutputStream(uri)) {
            out.write(survey.toJson().getBytes(StandardCharsets.UTF_8));
        } catch (Exception e) {
            Toast.makeText(this, "Couldn't save: " + e.getMessage(), Toast.LENGTH_LONG).show();
            return;
        }
        Toast.makeText(this, "Saved " + name + " in Downloads.", Toast.LENGTH_LONG).show();
        Intent send = new Intent(Intent.ACTION_SEND);
        send.setType("application/octet-stream");
        send.putExtra(Intent.EXTRA_STREAM, uri);
        send.putExtra(Intent.EXTRA_SUBJECT, "Wi-Fi survey for NetSketch");
        send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        startActivity(Intent.createChooser(send, "Send the survey to your computer"));
    }
}
