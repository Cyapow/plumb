package com.plumb

import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/**
 * Finds or starts the shared `plumb serve` agent, mirroring the VS Code
 * extension: reuse a live advertised server (pid alive + healthy), else spawn a
 * detached one and wait for it to advertise itself in the discovery file.
 */
object PlumbAgent {

    data class Disc(val port: Int, val token: String, val pid: Long)

    private fun discoveryFile(): File {
        val home = System.getProperty("user.home")
        val os = System.getProperty("os.name").lowercase()
        return when {
            os.contains("mac") -> File(home, "Library/Application Support/plumb/serve.json")
            os.contains("win") -> File(System.getenv("APPDATA") ?: home, "plumb/serve.json")
            else -> File(System.getenv("XDG_CONFIG_HOME") ?: "$home/.config", "plumb/serve.json")
        }
    }

    private fun readDiscovery(): Disc? {
        val f = discoveryFile()
        if (!f.exists()) return null
        val t = try { f.readText() } catch (e: Exception) { return null }
        val port = Regex("\"port\"\\s*:\\s*(\\d+)").find(t)?.groupValues?.get(1)?.toIntOrNull() ?: return null
        val pid = Regex("\"pid\"\\s*:\\s*(\\d+)").find(t)?.groupValues?.get(1)?.toLongOrNull() ?: 0L
        val token = Regex("\"token\"\\s*:\\s*\"([^\"]+)\"").find(t)?.groupValues?.get(1) ?: ""
        return Disc(port, token, pid)
    }

    private fun pidAlive(pid: Long): Boolean =
        try { ProcessHandle.of(pid).map { it.isAlive }.orElse(false) } catch (e: Exception) { false }

    private fun health(port: Int): Boolean = try {
        val c = URL("http://127.0.0.1:$port/").openConnection() as HttpURLConnection
        c.connectTimeout = 800
        c.readTimeout = 800
        c.requestMethod = "GET"
        val ok = c.responseCode in 200..499
        c.disconnect()
        ok
    } catch (e: Exception) {
        false
    }

    private fun binaryPath(): String {
        System.getenv("PLUMB_BIN")?.let { if (it.isNotBlank()) return it }
        val os = System.getProperty("os.name").lowercase()
        return when {
            os.contains("mac") -> "/Applications/Plumb.app/Contents/MacOS/plumb"
            os.contains("win") -> "plumb.exe"
            else -> "plumb"
        }
    }

    /** Return the port of a live agent, reusing one if advertised, else spawning it. */
    fun ensureServer(projectDir: String): Int {
        readDiscovery()?.let { if (pidAlive(it.pid) && health(it.port)) return it.port }

        ProcessBuilder(binaryPath(), "serve", projectDir)
            .redirectOutput(ProcessBuilder.Redirect.DISCARD)
            .redirectError(ProcessBuilder.Redirect.DISCARD)
            .start()

        val deadline = System.currentTimeMillis() + 15000
        while (System.currentTimeMillis() < deadline) {
            readDiscovery()?.let { if (pidAlive(it.pid) && health(it.port)) return it.port }
            Thread.sleep(300)
        }
        throw RuntimeException("`plumb serve` didn't start. Set the PLUMB_BIN env var to a Plumb build that supports serve mode (v0.10.7+).")
    }
}
