// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.app.Dialog
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.ScrollView

/**
 * A full-screen list over the activity its [Ui] was made for: a top bar, a search field, and rows
 * in groups. The search keeps the rows whose title or supporting text contains the query, and the
 * groups that still have one.
 */
class ListScreen
internal constructor(
    private val ui: Ui,
    private val title: CharSequence?,
    private val searchHint: CharSequence?,
) {
    private class Group(val label: CharSequence?, val items: List<ListItem>)

    private val groups = mutableListOf<Group>()
    private var dismissed: Runnable? = null
    private var body: LinearLayout? = null
    private var query = ""

    /**
     * Adds [items] as one group under [label], or without a heading when it is null. An open screen
     * shows it at once.
     */
    fun group(label: CharSequence?, items: List<ListItem>): ListScreen = apply {
        groups += Group(label, items)
        body?.let(::fill)
    }

    fun clear(): ListScreen = apply {
        groups.clear()
        body?.let(::fill)
    }

    fun onDismiss(listener: Runnable): ListScreen = apply { dismissed = listener }

    fun show() {
        lateinit var dialog: Dialog
        dialog =
            fullScreenDialog(
                activity(ui.context),
                back = { dialog.dismiss() },
                closed = {
                    body = null
                    dismissed?.run()
                },
            )
        val body =
            LinearLayout(ui.context).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(
                    ui.px(Tokens.SPACE_LG),
                    0,
                    ui.px(Tokens.SPACE_LG),
                    ui.px(Tokens.SPACE_XXL),
                )
            }
        query = ""
        val search =
            ui.searchBar(searchHint).apply {
                onQueryChange {
                    query = it
                    fill(body)
                }
            }
        this.body = body
        val page =
            PageLayout(ui).apply {
                addView(ui.topBar(title) { dialog.dismiss() }, matchWidth())
                addView(
                    search,
                    matchWidth().apply {
                        marginStart = ui.px(Tokens.SPACE_LG)
                        marginEnd = ui.px(Tokens.SPACE_LG)
                        bottomMargin = ui.px(Tokens.SPACE_SM)
                    },
                )
                addView(
                    ScrollView(ui.context).apply { addView(body) },
                    LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f),
                )
            }
        dialog.setContentView(page)
        checkNotNull(dialog.window) { "A dialog always owns a window" }
            .apply {
                setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
                paintSurface(ui)
            }
        fill(body)
        dialog.show()
    }

    private fun fill(body: LinearLayout) {
        body.removeAllViews()
        groups.forEach { group ->
            val shown = group.items.filter { it.matches(query) }
            if (shown.isEmpty()) return@forEach
            group.label?.let { body.addView(ui.sectionLabel(it)) }
            val items = ui.itemGroup()
            shown.forEach {
                (it.parent as ViewGroup?)?.removeView(it)
                items.add(it)
            }
            body.addView(items, matchWidth())
        }
    }

    private fun matchWidth() =
        LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        )
}
