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

/** The look: the same dark slate and teal as NetSketch on the computer. */
final class Ui {
    static final int BG = Color.rgb(11, 18, 32);
    static final int CARD = Color.rgb(21, 31, 50);
    static final int LINE = Color.rgb(36, 50, 74);
    static final int TEXT = Color.rgb(230, 237, 247);
    static final int DIM = Color.rgb(138, 154, 181);
    static final int ACCENT = Color.rgb(45, 212, 191);
    static final int AMBER = Color.rgb(245, 158, 11);
    static final int RED = Color.rgb(239, 68, 68);
    static final int GREEN = Color.rgb(34, 197, 94);
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
        b.setBackground(new RippleDrawable(ColorStateList.valueOf(primary ? Color.argb(70, 0, 0, 0) : Color.argb(50, 255, 255, 255)), shape, null));
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
        if (rssi >= -70) return Color.rgb(163, 190, 40);
        if (rssi >= -80) return AMBER;
        return RED;
    }

    static String qualityWord(double rssi) {
        if (rssi >= -55) return "excellent";
        if (rssi >= -65) return "good";
        if (rssi >= -72) return "fair";
        if (rssi >= -80) return "weak";
        return "very weak";
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
}
