package io.github.kuddev.pebrel.mobile.connection

import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.*
import java.io.ByteArrayInputStream

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [28], application = android.app.Application::class)
@ConscryptMode(ConscryptMode.Mode.OFF)
class TmuxSessionControlTest {
    @Test fun touchScrollUsesTheCurrentClientsCopyModeAndInputCancelsItWithoutSendingShellKeys() {
        val commands = mutableListOf<String>()
        val targets = mutableListOf<RemoteAttachment>()
        var mode = ""
        var window = "@4"
        val controller = TmuxSessionControl({ command ->
            commands += command
            JSONObject().put("status", 0).put("stdout", if (command.contains("list-clients"))
                "/dev/pts/other\t${'$'}99\tOther\t10:20\t@99\tOther\t%99\t\n" +
                "/dev/pts/7\t${'$'}3\tWork\t10:20\t$window\tEditor\t%5\t$mode\n" else "")
        }, targets::add)
        val wrapped = controller.command("exec tmux attach-session -t 'work'")
        val token = Regex("PEBREL_TTY_([a-f0-9]+)=").find(wrapped)!!.groupValues[1]
        controller.input(ByteArrayInputStream("\u001ePEBREL_TTY_$token=/dev/pts/7\u001foutput".toByteArray())).readBytes()
        controller.scroll(-3)
        assertTrue(commands.last().contains("copy-mode"))
        assertTrue(commands.last().contains("scroll-up"))
        assertTrue(commands.last().contains("-N 3"))
        assertFalse(commands.last().contains("%99"))
        mode = "copy-mode"
        controller.returnToInput()
        assertTrue(commands.last().contains("cancel"))
        assertTrue(commands.last().contains("${'$'}3:@4.%5"))
        window = "@8"
        assertEquals("@8", controller.checkpoint()?.window?.id)
        assertEquals("@8", targets.last().window?.id)
    }

    @Test fun noMarkerOrAnotherClientCannotSelectAnArbitraryTmuxSession() {
        var queried = false
        val controller = TmuxSessionControl({ queried = true; error("no tty") }, {})
        controller.scroll(-1)
        assertFalse(queried)
        assertNull(TmuxSessionControl.parseClient("/dev/pts/other\t${'$'}3\tWork\t10:20\t@4\tEditor\t%5\t", "/dev/pts/7"))
        assertNull(TmuxSessionControl.parseClient("/dev/pts/7\t${'$'}3\tWork\t10:20\t@4\tEditor\t%5;bad\t", "/dev/pts/7"))
    }
}
