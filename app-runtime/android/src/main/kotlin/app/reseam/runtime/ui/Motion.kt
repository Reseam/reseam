// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.animation.Animator
import android.animation.AnimatorSet
import android.animation.ObjectAnimator
import android.util.Property
import android.view.View
import android.view.animation.Interpolator
import android.view.animation.PathInterpolator

/** Material 3 motion matching Reseam Manager navigation. */
object Motion {
    private const val MEDIUM2 = 300L
    private const val FADE_OUT = MEDIUM2 * 35 / 100
    private const val SLIDE = 30
    private val emphasized = PathInterpolator(0.2f, 0f, 0f, 1f)
    private val decelerate = PathInterpolator(0.05f, 0.7f, 0.1f, 1f)
    private val accelerate = PathInterpolator(0.3f, 0f, 0.8f, 0.15f)

    /**
     * Slides screens 30dp toward the start when forward, fading the outgoing screen before the
     * incoming one.
     */
    @JvmStatic
    fun sharedAxisX(outgoing: View, incoming: View, forward: Boolean): Animator {
        val slide = Tokens.px(incoming.context, SLIDE.toFloat()).toFloat() * if (forward) 1 else -1
        // A delayed animator applies its start value only once the delay ends.
        incoming.alpha = 0f
        incoming.translationX = slide
        return AnimatorSet().apply {
            playTogether(
                animate(incoming, View.TRANSLATION_X, slide, 0f, 0, MEDIUM2, emphasized),
                animate(incoming, View.ALPHA, 0f, 1f, FADE_OUT, MEDIUM2 - FADE_OUT, decelerate),
                animate(outgoing, View.TRANSLATION_X, 0f, -slide, 0, MEDIUM2, emphasized),
                animate(outgoing, View.ALPHA, 1f, 0f, 0, FADE_OUT, accelerate),
            )
        }
    }

    private fun animate(
        view: View,
        property: Property<View, Float>,
        from: Float,
        to: Float,
        delay: Long,
        duration: Long,
        interpolator: Interpolator,
    ): Animator =
        ObjectAnimator.ofFloat(view, property, from, to).apply {
            startDelay = delay
            this.duration = duration
            this.interpolator = interpolator
        }
}
