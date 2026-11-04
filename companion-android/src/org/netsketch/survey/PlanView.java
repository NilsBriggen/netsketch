package org.netsketch.survey;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.view.MotionEvent;
import android.view.View;

import java.util.ArrayList;
import java.util.List;

/**
 * The floor plan from NetSketch: walls, room names, devices and earlier measuring points of one storey.
 * Tap where you stand; that spot (in metres, like the plan) goes with the next measurement.
 */
final class PlanView extends View {
    static final class Wall {
        int floor;
        float ax, ay, bx, by, thick;
        int color;
    }

    static final class Label {
        int floor;
        float x, y;
        String text;
    }

    static final class Dev {
        int floor;
        float x, y;
        String name;
        boolean radio;
    }

    static final class Pin {
        int floor;
        float x, y;
        String name;
    }

    interface OnSpot {
        void picked(float x, float y);
    }

    final List<Wall> walls = new ArrayList<>();
    final List<Label> labels = new ArrayList<>();
    final List<Dev> devices = new ArrayList<>();
    final List<Pin> pins = new ArrayList<>();
    int floor;
    /** The spot picked for the next measurement (metres), or null. */
    Float selX, selY;
    OnSpot onSpot;

    private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
    private float scale = 1, offX, offY;
    private float downX, downY;

    PlanView(Context c) {
        super(c);
        setBackground(Ui.round(c, Ui.BG, 14, Ui.LINE));
    }

    void clear() {
        walls.clear();
        labels.clear();
        devices.clear();
        pins.clear();
        selX = null;
        selY = null;
    }

    /** Fits this storey into the view, with room around the edges. */
    private void fit() {
        float x0 = Float.MAX_VALUE, y0 = Float.MAX_VALUE, x1 = -Float.MAX_VALUE, y1 = -Float.MAX_VALUE;
        for (Wall w : walls) {
            if (w.floor != floor) continue;
            x0 = Math.min(x0, Math.min(w.ax, w.bx));
            x1 = Math.max(x1, Math.max(w.ax, w.bx));
            y0 = Math.min(y0, Math.min(w.ay, w.by));
            y1 = Math.max(y1, Math.max(w.ay, w.by));
        }
        for (Dev d : devices) {
            if (d.floor != floor) continue;
            x0 = Math.min(x0, d.x);
            x1 = Math.max(x1, d.x);
            y0 = Math.min(y0, d.y);
            y1 = Math.max(y1, d.y);
        }
        if (x0 > x1) {
            x0 = 0;
            y0 = 0;
            x1 = 10;
            y1 = 8;
        }
        float pad = Ui.dp(getContext(), 22);
        float w = Math.max(1, x1 - x0), h = Math.max(1, y1 - y0);
        scale = Math.min((getWidth() - 2 * pad) / w, (getHeight() - 2 * pad) / h);
        offX = (getWidth() - w * scale) / 2 - x0 * scale;
        offY = (getHeight() - h * scale) / 2 - y0 * scale;
    }

    private float sx(float x) {
        return offX + x * scale;
    }

    private float sy(float y) {
        return offY + y * scale;
    }

    @Override
    protected void onDraw(Canvas cv) {
        fit();
        p.setStrokeCap(Paint.Cap.ROUND);
        for (Wall w : walls) {
            if (w.floor != floor) continue;
            p.setStyle(Paint.Style.STROKE);
            p.setColor(w.color);
            p.setStrokeWidth(Math.max(Ui.dp(getContext(), 2), w.thick * scale));
            cv.drawLine(sx(w.ax), sy(w.ay), sx(w.bx), sy(w.by), p);
        }
        p.setStyle(Paint.Style.FILL);
        p.setTextAlign(Paint.Align.CENTER);
        p.setTextSize(Ui.dp(getContext(), 12));
        p.setColor(Ui.DIM);
        for (Label l : labels) {
            if (l.floor == floor) cv.drawText(l.text, sx(l.x), sy(l.y), p);
        }
        float r = Ui.dp(getContext(), 5);
        p.setTextSize(Ui.dp(getContext(), 9));
        for (Dev d : devices) {
            if (d.floor != floor) continue;
            p.setColor(d.radio ? Ui.ACCENT : Color.rgb(96, 165, 250));
            cv.drawCircle(sx(d.x), sy(d.y), r, p);
            p.setColor(Ui.DIM);
            cv.drawText(d.name, sx(d.x), sy(d.y) + r + Ui.dp(getContext(), 10), p);
        }
        for (Pin q : pins) {
            if (q.floor != floor) continue;
            p.setColor(Ui.AMBER);
            cv.drawCircle(sx(q.x), sy(q.y), r * 0.8f, p);
        }
        if (selX != null && selY != null) {
            float x = sx(selX), y = sy(selY);
            p.setColor(Color.argb(60, 45, 212, 191));
            cv.drawCircle(x, y, r * 4, p);
            p.setStyle(Paint.Style.STROKE);
            p.setStrokeWidth(Ui.dp(getContext(), 2.5f));
            p.setColor(Ui.ACCENT);
            cv.drawCircle(x, y, r * 2.2f, p);
            p.setStyle(Paint.Style.FILL);
            cv.drawCircle(x, y, r * 0.9f, p);
        } else if (!walls.isEmpty()) {
            p.setColor(Ui.DIM);
            p.setTextSize(Ui.dp(getContext(), 12));
            cv.drawText("Tap where you're standing", getWidth() / 2f, getHeight() - Ui.dp(getContext(), 10), p);
        }
    }

    @Override
    public boolean onTouchEvent(MotionEvent e) {
        switch (e.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                downX = e.getX();
                downY = e.getY();
                getParent().requestDisallowInterceptTouchEvent(true);
                return true;
            case MotionEvent.ACTION_UP:
                getParent().requestDisallowInterceptTouchEvent(false);
                if (Math.hypot(e.getX() - downX, e.getY() - downY) < Ui.dp(getContext(), 12) && scale > 0) {
                    selX = (e.getX() - offX) / scale;
                    selY = (e.getY() - offY) / scale;
                    invalidate();
                    if (onSpot != null) onSpot.picked(selX, selY);
                    performClick();
                }
                return true;
            case MotionEvent.ACTION_CANCEL:
                getParent().requestDisallowInterceptTouchEvent(false);
                return true;
            default:
                return true;
        }
    }

    @Override
    public boolean performClick() {
        return super.performClick();
    }

    /** The room name nearest to a spot on this storey (within 6 m), for naming the point. */
    String roomAt(float x, float y) {
        String best = null;
        double bd = 6;
        for (Label l : labels) {
            if (l.floor != floor) continue;
            double d = Math.hypot(l.x - x, l.y - y);
            if (d < bd) {
                bd = d;
                best = l.text;
            }
        }
        return best;
    }

    static int wallColor(String kind) {
        switch (kind) {
            case "Concrete": return Color.rgb(120, 130, 145);
            case "Brick": return Color.rgb(190, 90, 70);
            case "Wood": return Color.rgb(180, 130, 80);
            case "Glass": case "LowE": return Color.rgb(125, 211, 252);
            case "Metal": return Color.rgb(200, 200, 210);
            default: return Color.rgb(160, 170, 185);
        }
    }

    static float wallThick(String kind) {
        switch (kind) {
            case "Concrete": return 0.25f;
            case "Brick": return 0.2f;
            case "Drywall": return 0.1f;
            case "Wood": return 0.08f;
            default: return 0.05f;
        }
    }
}
