package com.plumb

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory
import com.intellij.ui.jcef.JBCefApp
import com.intellij.ui.jcef.JBCefBrowser
import java.awt.BorderLayout
import java.net.URLEncoder
import javax.swing.JLabel
import javax.swing.JPanel
import javax.swing.SwingConstants

/**
 * Hosts the Plumb UI in a JCEF browser tool window. Starts (or reuses) the
 * shared serve agent on a background thread, then points the browser at the
 * served frontend with this project's directory as the repo.
 */
class PlumbToolWindowFactory : ToolWindowFactory {

    override fun isApplicable(project: Project): Boolean = JBCefApp.isSupported()

    override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
        val panel = JPanel(BorderLayout())

        if (!JBCefApp.isSupported()) {
            panel.add(JLabel("This IDE build doesn't include JCEF, so the Plumb panel can't render here.", SwingConstants.CENTER), BorderLayout.CENTER)
            addContent(toolWindow, panel)
            return
        }

        val browser = JBCefBrowser()
        browser.loadHTML("<html><body style='background:#161514;color:#888;font-family:sans-serif;padding:24px'>Starting Plumb…</body></html>")
        panel.add(browser.component, BorderLayout.CENTER)
        addContent(toolWindow, panel)

        val dir = project.basePath ?: System.getProperty("user.home")
        ApplicationManager.getApplication().executeOnPooledThread {
            try {
                val port = PlumbAgent.ensureServer(dir)
                val url = "http://127.0.0.1:$port/?repo=" + URLEncoder.encode(dir, "UTF-8")
                ApplicationManager.getApplication().invokeLater { browser.loadURL(url) }
            } catch (e: Exception) {
                val msg = (e.message ?: "unknown error").replace("<", "&lt;")
                ApplicationManager.getApplication().invokeLater {
                    browser.loadHTML("<html><body style='background:#161514;color:#e0663a;font-family:sans-serif;padding:24px'>Couldn't start Plumb: $msg</body></html>")
                }
            }
        }

        com.intellij.openapi.util.Disposer.register(toolWindow.disposable, browser)
    }

    private fun addContent(toolWindow: ToolWindow, panel: JPanel) {
        val content = ContentFactory.getInstance().createContent(panel, "", false)
        toolWindow.contentManager.addContent(content)
    }
}
