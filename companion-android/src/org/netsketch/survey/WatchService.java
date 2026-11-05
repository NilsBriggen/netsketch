package org.netsketch.survey;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.ServiceInfo;
import android.net.Network;
import android.net.wifi.WifiInfo;
import android.net.wifi.WifiManager;
import android.os.Build;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.IBinder;
import android.os.PowerManager;
import android.os.SystemClock;

import java.io.File;
import java.net.InetAddress;

/**
 * "Watch my Wi-Fi" in the background: a foreground service (type location, so Android lets it read the
 * Wi-Fi and scan while the screen is off) that takes a reading every second and a scan whenever the scan
 * budget allows, into the shared watch log. If Android refuses the foreground part, it still runs while
 * the app is open (the screen then stays on, see MainActivity).
 */
public final class WatchService extends Service {
    static final String STOP = "org.netsketch.survey.STOP";
    private static final String CHANNEL = "watch";
    /** Scans while watching: at most one every 40 s, leaving room for measuring at the same time. */
    private static final long SCAN_EVERY_MS = 40_000;

    static volatile boolean running, foreground;

    private HandlerThread thread;
    private Handler work;
    private PowerManager.WakeLock wake;
    private BroadcastReceiver scans;
    private WifiManager wifi;
    private Network net;
    private InetAddress gw;
    private long next, lastScan, lastRoute, lastNote, lastPrune;

    /**
     * Started as a plain service from the open app, which then makes itself a foreground one: if Android
     * refuses that, it simply runs on while the app is open (startForegroundService would crash instead).
     */
    static void start(Context c) {
        c.startService(new Intent(c, WatchService.class));
    }

    static void stop(Context c) {
        c.stopService(new Intent(c, WatchService.class));
    }

    /** The log file (shared with the screen). */
    static File log(Context c) {
        return new File(c.getFilesDir(), "watch.log");
    }

    @Override
    public void onCreate() {
        super.onCreate();
        running = true;
        wifi = getApplicationContext().getSystemService(WifiManager.class);
        Watch w = Watch.get();
        w.open(log(this), System.currentTimeMillis());
        w.startedAt = System.currentTimeMillis();
        w.active = true;
        scans = new BroadcastReceiver() {
            @Override
            public void onReceive(Context c, Intent i) {
                Scope.feed(WatchService.this, wifi, false);
            }
        };
        IntentFilter f = new IntentFilter(WifiManager.SCAN_RESULTS_AVAILABLE_ACTION);
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(scans, f, Context.RECEIVER_NOT_EXPORTED);
        else registerReceiver(scans, f);
        // A reading every second also with the screen off; let go after a day at the latest.
        wake = getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "netsketch:watch");
        wake.acquire(Watch.KEEP_MS);
        thread = new HandlerThread("watch");
        thread.start();
        work = new Handler(thread.getLooper());
        next = SystemClock.uptimeMillis();
        work.post(this::tick);
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && STOP.equals(intent.getAction())) {
            stopSelf();
            return START_NOT_STICKY;
        }
        try {
            NotificationManager nm = getSystemService(NotificationManager.class);
            nm.createNotificationChannel(new NotificationChannel(CHANNEL, T.t("Watching your Wi-Fi"), NotificationManager.IMPORTANCE_LOW));
            if (Build.VERSION.SDK_INT >= 29) startForeground(1, note(T.t("Starting…")), ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION);
            else startForeground(1, note(T.t("Starting…")));
            foreground = true;
        } catch (RuntimeException e) {
            // Not allowed (permission missing, or started from the background): it carries on while the app is open.
            foreground = false;
        }
        return START_NOT_STICKY;
    }

    private Notification note(String text) {
        Intent open = new Intent(this, MainActivity.class).putExtra("page", 2).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent tap = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent stop = PendingIntent.getService(this, 1, new Intent(this, WatchService.class).setAction(STOP), PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this, CHANNEL)
                .setSmallIcon(R.drawable.ic_launcher)
                .setContentTitle(T.t("Watching your Wi-Fi"))
                .setContentText(text)
                .setOngoing(true)
                .setContentIntent(tap)
                .addAction(new Notification.Action.Builder(null, T.t("Stop"), stop).build())
                .build();
    }

    /** One reading: signal, access point, link rates and a knock on the router; a scan when allowed. */
    @SuppressWarnings("deprecation")
    private void tick() {
        if (!running) return;
        long now = SystemClock.elapsedRealtime();
        if (net == null || now - lastRoute > 10_000) {
            net = Scope.wifiNetwork(this);
            gw = Scope.gateway(this, net);
            lastRoute = now;
        }
        Watch.Sample s = new Watch.Sample();
        s.at = System.currentTimeMillis();
        WifiInfo i = wifi.getConnectionInfo();
        // Without location Android hides the address (02:00:00:00:00:00), but the connection is real.
        if (i != null && i.getRssi() > -127 && i.getFrequency() > 0 && i.getBSSID() != null) {
            s.rssi = i.getRssi();
            s.bssid = i.getBSSID();
            String ssid = i.getSSID();
            s.ssid = ssid == null || ssid.equals(WifiManager.UNKNOWN_SSID) ? "" : ssid.replaceAll("^\"|\"$", "");
            s.tx = i.getTxLinkSpeedMbps();
            s.rx = i.getRxLinkSpeedMbps();
            Double ms = net == null || gw == null ? null : Scope.knock(net, gw, 800);
            s.ping = ms == null ? Watch.LOST : ms;
        } else {
            net = null; // look for the network again when it's back
        }
        Watch w = Watch.get();
        w.add(s);
        if (now - lastScan >= SCAN_EVERY_MS && Scope.scansLeft(wifi) >= 2) {
            lastScan = now;
            Scope.startScan(wifi);
        }
        if (now - lastNote >= 10_000 && foreground) {
            lastNote = now;
            int events = w.events().size();
            String text = s.online() ? T.f("%d dBm · %d events in the log", s.rssi, events) : T.f("Wi-Fi is off or gone · %d events in the log", events);
            getSystemService(NotificationManager.class).notify(1, note(text));
        }
        if (now - lastPrune >= 3600_000) {
            lastPrune = now;
            w.prune(s.at);
        }
        // Keep to a steady second, also when a knock took a while.
        next += 1000;
        if (next < SystemClock.uptimeMillis()) next = SystemClock.uptimeMillis() + 200;
        work.postAtTime(this::tick, next);
    }

    @Override
    public void onDestroy() {
        running = false;
        foreground = false;
        Watch w = Watch.get();
        w.active = false;
        if (work != null) work.removeCallbacksAndMessages(null);
        if (thread != null) thread.quitSafely();
        w.finish(System.currentTimeMillis());
        try {
            unregisterReceiver(scans);
        } catch (IllegalArgumentException ignored) {
            // already gone
        }
        if (wake != null && wake.isHeld()) wake.release();
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
