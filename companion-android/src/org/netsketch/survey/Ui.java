package org.netsketch.survey;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.text.InputType;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;

/** The look: the same slate and teal as NetSketch on the computer, dark or light like the phone. */
final class Ui {
    static int BG, CARD, LINE, TEXT, DIM, ACCENT, AMBER, RED, GREEN, BLUE;
    static boolean dark;

    static {
        palette(true);
    }

    /** Picks the dark or light colours from the phone's setting; call before building the screen. */
    static void init(Context c) {
        palette((c.getResources().getConfiguration().uiMode & android.content.res.Configuration.UI_MODE_NIGHT_MASK) != android.content.res.Configuration.UI_MODE_NIGHT_NO);
    }

    private static void palette(boolean d) {
        dark = d;
        BG = d ? Color.rgb(11, 18, 32) : Color.rgb(243, 246, 250);
        CARD = d ? Color.rgb(21, 31, 50) : Color.WHITE;
        LINE = d ? Color.rgb(36, 50, 74) : Color.rgb(214, 222, 233);
        TEXT = d ? Color.rgb(230, 237, 247) : Color.rgb(15, 23, 42);
        DIM = d ? Color.rgb(138, 154, 181) : Color.rgb(84, 99, 122);
        ACCENT = d ? Color.rgb(45, 212, 191) : Color.rgb(13, 148, 136);
        AMBER = d ? Color.rgb(245, 158, 11) : Color.rgb(180, 105, 0);
        RED = d ? Color.rgb(239, 68, 68) : Color.rgb(200, 30, 30);
        GREEN = d ? Color.rgb(34, 197, 94) : Color.rgb(21, 128, 61);
        BLUE = d ? Color.rgb(96, 165, 250) : Color.rgb(37, 99, 235);
    }
    static final Typeface MEDIUM = Typeface.create("sans-serif-medium", Typeface.NORMAL);

    private Ui() {}

    static int dp(Context c, float v) {
        return Math.round(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v, c.getResources().getDisplayMetrics()));
    }

    static GradientDrawable round(Context c, int fill, float radius, int stroke) {
        GradientDrawable g = new GradientDrawable();
        g.setColor(fill);
        g.setCornerRadius(dp(c, radius));
        if (stroke != 0) g.setStroke(dp(c, 1), stroke);
        return g;
    }

    static TextView text(Context c, String s, float sp, int color) {
        TextView t = new TextView(c);
        t.setText(s);
        t.setTextSize(sp);
        t.setTextColor(color);
        t.setLineSpacing(0, 1.15f);
        return t;
    }

    static TextView title(Context c, String s, float sp) {
        TextView t = text(c, s, sp, TEXT);
        t.setTypeface(MEDIUM);
        return t;
    }

    /** A rounded card with a little padding; add children to it. */
    static LinearLayout card(Context c) {
        LinearLayout l = new LinearLayout(c);
        l.setOrientation(LinearLayout.VERTICAL);
        l.setBackground(round(c, CARD, 16, 0));
        int p = dp(c, 16);
        l.setPadding(p, p, p, p);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        lp.topMargin = dp(c, 12);
        l.setLayoutParams(lp);
        return l;
    }

    /** Small uppercase heading inside a card. */
    static TextView heading(Context c, String s) {
        TextView t = text(c, s.toUpperCase(), 12, DIM);
        t.setLetterSpacing(0.08f);
        t.setPadding(0, 0, 0, dp(c, 8));
        return t;
    }

    static Button button(Context c, String label, boolean primary) {
        Button b = new Button(c);
        b.setText(label);
        b.setAllCaps(false);
        b.setTextSize(15);
        b.setTypeface(MEDIUM);
        b.setTextColor(primary ? BG : TEXT);
        b.setStateListAnimator(null);
        GradientDrawable shape = round(c, primary ? ACCENT : CARD, 14, primary ? 0 : LINE);
        // The press ripple: dark on the teal and on light cards, light on dark ones.
        int ripple = primary || !dark ? Color.argb(primary ? 70 : 40, 0, 0, 0) : Color.argb(50, 255, 255, 255);
        b.setBackground(new RippleDrawable(ColorStateList.valueOf(ripple), shape, null));
        b.setPadding(dp(c, 16), dp(c, 10), dp(c, 16), dp(c, 10));
        return b;
    }

    /** A pill-shaped status or choice chip. */
    static TextView chip(Context c, String s, int color, boolean filled) {
        TextView t = text(c, s, 13, filled ? BG : color);
        t.setTypeface(MEDIUM);
        t.setBackground(round(c, filled ? color : Color.TRANSPARENT, 20, filled ? 0 : color));
        t.setPadding(dp(c, 12), dp(c, 5), dp(c, 12), dp(c, 5));
        t.setGravity(Gravity.CENTER);
        return t;
    }

    static EditText field(Context c, String hint) {
        EditText e = new EditText(c);
        e.setHint(hint);
        e.setSingleLine(true);
        e.setTextColor(TEXT);
        e.setHintTextColor(DIM);
        e.setTextSize(16);
        e.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        e.setBackground(round(c, BG, 12, LINE));
        e.setPadding(dp(c, 14), dp(c, 12), dp(c, 14), dp(c, 12));
        return e;
    }

    static View gap(Context c, int dp) {
        View v = new View(c);
        v.setLayoutParams(new LinearLayout.LayoutParams(1, dp(c, dp)));
        return v;
    }

    /** Colour for a signal strength (dBm), as on the computer's Wi-Fi map. */
    static int quality(double rssi) {
        if (rssi >= -60) return GREEN;
        if (rssi >= -70) return dark ? Color.rgb(163, 190, 40) : Color.rgb(101, 130, 0);
        if (rssi >= -80) return AMBER;
        return RED;
    }

    static String qualityWord(double rssi) {
        if (rssi >= -55) return T.t("excellent");
        if (rssi >= -65) return T.t("good");
        if (rssi >= -72) return T.t("fair");
        if (rssi >= -80) return T.t("weak");
        return T.t("very weak");
    }

    /** Four little bars, filled by signal strength. */
    static final class Bars extends View {
        private double rssi = -100;
        private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);

        Bars(Context c) {
            super(c);
            setLayoutParams(new LinearLayout.LayoutParams(dp(c, 22), dp(c, 18)));
        }

        void set(double rssi) {
            this.rssi = rssi;
            setContentDescription(T.f("Signal: %s", qualityWord(rssi)));
            invalidate();
        }

        @Override
        protected void onDraw(Canvas cv) {
            int lit = rssi >= -60 ? 4 : rssi >= -70 ? 3 : rssi >= -80 ? 2 : rssi >= -90 ? 1 : 0;
            float w = getWidth() / 5.5f, h = getHeight();
            for (int i = 0; i < 4; i++) {
                p.setColor(i < lit ? quality(rssi) : LINE);
                float top = h - h * (i + 1) / 4f;
                cv.drawRoundRect(i * w * 1.4f, top, i * w * 1.4f + w, h, w / 3, w / 3, p);
            }
        }
    }

    /** The last minute of the live signal, as a small line (newest on the right). */
    static final class Spark extends View {
        private final float[] v = new float[60];
        private int n;
        private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);

        Spark(Context c) {
            super(c);
        }

        void add(double rssi) {
            if (n == v.length) System.arraycopy(v, 1, v, 0, --n);
            v[n++] = (float) rssi;
            invalidate();
        }

        /** {lowest, highest} of the last minute, or null before the first reading. */
        float[] range() {
            if (n == 0) return null;
            float lo = Float.MAX_VALUE, hi = -Float.MAX_VALUE;
            for (int i = 0; i < n; i++) {
                lo = Math.min(lo, v[i]);
                hi = Math.max(hi, v[i]);
            }
            return new float[] {lo, hi};
        }

        @Override
        protected void onDraw(Canvas cv) {
            Context c = getContext();
            // The scale follows the last minute (at least 15 dB, on 5 dB steps), so the labelled lines frame the curve.
            float[] r = range();
            hi = r == null ? -40 : (float) Math.ceil((r[1] + 2) / 5) * 5;
            lo = r == null ? -90 : (float) Math.floor((r[0] - 2) / 5) * 5;
            if (hi - lo < 15) lo = hi - 15;
            int every = hi - lo > 30 ? 10 : 5;
            p.setTextSize(dp(c, 10));
            float pad = dp(c, 6), left = p.measureText("-100") + dp(c, 6), w = getWidth();
            top = pad;
            span = getHeight() - 2 * pad;
            for (int db = (int) hi; db >= lo; db -= every) {
                float y = y(db);
                p.setStyle(Paint.Style.FILL);
                p.setColor(DIM);
                cv.drawText(String.valueOf(db), 0, y + p.getTextSize() / 3, p);
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(c, 1));
                p.setColor(LINE);
                cv.drawLine(left, y, w, y, p);
            }
            if (n < 2) return;
            p.setStrokeWidth(dp(c, 2));
            p.setStrokeCap(Paint.Cap.ROUND);
            float step = (w - left) / (v.length - 1), x0 = w - (n - 1) * step;
            for (int i = 1; i < n; i++) {
                p.setColor(quality(v[i]));
                cv.drawLine(x0 + (i - 1) * step, y(v[i - 1]), x0 + i * step, y(v[i]), p);
            }
        }

        private float hi, lo, top, span;

        private float y(float rssi) {
            return top + span * Math.max(0, Math.min(1, (hi - rssi) / (hi - lo)));
        }
    }

    /**
     * The networks on one band as hills over the channel numbers, like a Wi-Fi analyser: width = channel
     * width, height = signal. Yours in teal, the others dim; overlaps show who shares airtime with whom.
     */
    static final class Channels extends View {
        final java.util.List<Survey.Seen> nets = new java.util.ArrayList<>();
        final java.util.Set<String> own = new java.util.HashSet<>();
        String band = "2.4 GHz";
        private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);

        Channels(Context c) {
            super(c);
        }

        @Override
        protected void onDraw(Canvas cv) {
            Context c = getContext();
            float w = getWidth(), h = getHeight(), bottom = h - dp(c, 18), top = dp(c, 14);
            // MHz span of the band, and the channel numbers to label.
            int[] span = Survey.chartSpan(band);
            float f0 = span[0], f1 = span[1];
            int[] ticks;
            if (band.equals("2.4 GHz")) ticks = new int[] {1, 6, 11, 13};
            else if (band.equals("5 GHz")) ticks = new int[] {36, 52, 100, 116, 132, 149, 165};
            else ticks = new int[] {1, 33, 65, 97, 129, 161, 193, 225};
            p.setTextSize(dp(c, 10));
            p.setTextAlign(Paint.Align.CENTER);
            p.setStyle(Paint.Style.FILL);
            p.setColor(DIM);
            for (int t : ticks) {
                int freq = band.equals("2.4 GHz") ? 2407 + 5 * t : band.equals("5 GHz") ? 5000 + 5 * t : 5950 + 5 * t;
                cv.drawText(String.valueOf(t), (freq - f0) / (f1 - f0) * w, h - dp(c, 4), p);
            }
            p.setStyle(Paint.Style.STROKE);
            p.setStrokeWidth(dp(c, 1));
            p.setColor(LINE);
            cv.drawLine(0, bottom, w, bottom, p);
            for (int pass = 0; pass < 2; pass++) {
                for (Survey.Seen s : nets) {
                    if (!Survey.bandOf(s.freq).equals(band)) continue;
                    boolean mine = own.contains(s.bssid);
                    if ((pass == 1) != mine) continue; // yours on top
                    // Centre of the whole (possibly bonded) channel: the scan reports the primary 20 MHz.
                    float centre = s.freq, half = s.width / 2f;
                    if (s.width > 20 && s.freq >= 5000) {
                        float base = s.freq >= 5925 ? 5955 : 5180;
                        float blockStart = base - 10 + (float) Math.floor((s.freq - (base - 10)) / s.width) * s.width;
                        centre = blockStart + half;
                    } else if (s.width == 40 && s.freq < 3000) {
                        centre = s.freq + 10;
                    }
                    float x0 = (centre - half - f0) / (f1 - f0) * w, x1 = (centre + half - f0) / (f1 - f0) * w;
                    float peak = bottom - (bottom - top) * (float) Math.max(0, Math.min(1, (s.rssi + 95) / 60));
                    android.graphics.Path path = new android.graphics.Path();
                    float edge = Math.min((x1 - x0) * 0.15f, dp(c, 8));
                    path.moveTo(x0, bottom);
                    path.lineTo(x0 + edge, peak);
                    path.lineTo(x1 - edge, peak);
                    path.lineTo(x1, bottom);
                    int col = mine ? ACCENT : DIM;
                    p.setStyle(Paint.Style.FILL);
                    p.setColor(Color.argb(mine ? 70 : 35, Color.red(col), Color.green(col), Color.blue(col)));
                    cv.drawPath(path, p);
                    p.setStyle(Paint.Style.STROKE);
                    p.setStrokeWidth(dp(c, mine ? 2 : 1));
                    p.setColor(col);
                    cv.drawPath(path, p);
                    if (mine || s.rssi >= -75) {
                        p.setStyle(Paint.Style.FILL);
                        p.setColor(mine ? ACCENT : DIM);
                        String name = s.ssid.isEmpty() ? T.t("(hidden)") : s.ssid;
                        cv.drawText(name.length() > 14 ? name.substring(0, 13) + "…" : name, Math.max(dp(c, 30), Math.min(w - dp(c, 30), (x0 + x1) / 2)), peak - dp(c, 3), p);
                    }
                }
            }
        }
    }
}
