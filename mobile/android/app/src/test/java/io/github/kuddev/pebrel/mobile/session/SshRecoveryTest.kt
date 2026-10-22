package io.github.kuddev.pebrel.mobile.session

import android.app.Application
import android.os.Looper
import androidx.test.core.app.ApplicationProvider
import io.github.kuddev.pebrel.mobile.connection.*
import io.github.kuddev.pebrel.terminal.*
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.*
import java.time.Duration

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [28], application = Application::class, shadows = [SshRecoveryTest.Terminal::class],
    instrumentedPackages = ["io.github.kuddev.pebrel.terminal"])
@ConscryptMode(ConscryptMode.Mode.OFF)
class SshRecoveryTest {
    @Implements(value = TerminalSession::class, isInAndroidSdk = false)
    class Terminal {
        @RealObject lateinit var real: TerminalSession
        lateinit var callbacks: TerminalCallbacks
        var closed = false
        @Implementation fun __constructor__(transport: SessionTransport, callbacks: TerminalCallbacks, scrollbackLines: Int) {
            this.callbacks = callbacks
        }
        @Implementation fun start() { started += this }
        @Implementation fun finishIfRunning() { closed = true }
        @Implementation fun colors(value: IntArray) = Unit
        companion object { val started = mutableListOf<Terminal>() }
    }

    private fun await(condition: () -> Boolean) {
        repeat(500) {
            shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(10))
            if (condition()) return
            Thread.sleep(10)
        }
        fail("session transition did not complete")
    }

    @Test fun retryReplacesOnlyTheTransportAndStaleCallbacksCannotCloseItsReplacement() {
        Terminal.started.clear()
        val repository = SessionRepository(ApplicationProvider.getApplicationContext())
        val host = HostProfile("fixture", "Fixture", "localhost", 22, "user")
        val target = RemoteAttachment(RemoteSession("tmux", "${'$'}3", "work", "10:20"), RemoteWindow("@4", "A", pane = "%5"))
        try {
            val a = repository.ssh(host, charArrayOf(), target)
            val old = Terminal.started.last()
            old.callbacks.onTransportReady(old.real)
            val b = repository.ssh(host, charArrayOf())
            val other = Terminal.started.last()
            other.callbacks.onTransportReady(other.real)
            repository.setDraft(a, "unfinished")
            old.callbacks.onSessionFinished(old.real)
            repository.retrySsh(a)
            await { Terminal.started.size == 3 }
            val replacement = Terminal.started.last()
            assertTrue(old.closed)
            assertFalse(other.closed)
            assertEquals(listOf(a, b), repository.sessions.value.map { it.id })
            assertEquals(target, repository.sessions.value.first().attachment)
            assertEquals("unfinished", repository.drafts.value[a])
            old.callbacks.onSessionFinished(old.real)
            old.callbacks.onTransportReady(old.real)
            assertEquals("connecting", repository.sessions.value.first().status)
            replacement.callbacks.onTransportReady(replacement.real)
            assertEquals("ready", repository.sessions.value.first().status)
            assertSame(other.real, repository.sessions.value.last().terminal)
        } finally { repository.closeAll() }
    }

    @Test fun routeRecoveryRunsInTheBackgroundAndClosingATabCancelsItsRetry() {
        Terminal.started.clear()
        val repository = SessionRepository(ApplicationProvider.getApplicationContext())
        try {
            val id = repository.ssh(HostProfile("fixture", "Fixture", "localhost", 22, "user"), charArrayOf())
            val first = Terminal.started.last()
            first.callbacks.onTransportReady(first.real)
            repository.foregroundChanged(false)
            repository.networkChanged()
            await { Terminal.started.size == 2 }
            assertEquals(id, repository.sessions.value.single().id)
            val second = Terminal.started.last()
            second.callbacks.onTransportReady(second.real)
            repository.networkChanged()
            repository.closeTerminal(id)
            shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(60))
            assertEquals(2, Terminal.started.size)
            assertTrue(repository.sessions.value.isEmpty())
        } finally { repository.closeAll() }
    }
}
