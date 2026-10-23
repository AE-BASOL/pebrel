# PSReadLine's native Paste inserts one edit without invoking per-character keys.
# Keep an existing user binding; advertise only while this editor can handle it.
function global:Get-PebrelClipboardBinding {
    $query = Get-Command Get-PSReadLineKeyHandler -ErrorAction Stop
    if ($query.Parameters.ContainsKey('Chord')) {
        Get-PSReadLineKeyHandler -Chord Ctrl+Shift+F11 -ErrorAction Stop
    } else {
        Get-PSReadLineKeyHandler | Where-Object { $_.Key -in 'Ctrl+Shift+F11','Shift+Ctrl+F11' }
    }
}

try {
    if ((Get-Command Set-PSReadLineKeyHandler -ErrorAction SilentlyContinue) -and
        (Get-PSReadLineOption -ErrorAction Stop).EditMode -ne 'Vi') {
        $clipboardBinding = Get-PebrelClipboardBinding
        if (-not $clipboardBinding -or $clipboardBinding.Function -eq 'Unbound') {
            Set-PSReadLineKeyHandler -Chord Ctrl+Shift+F11 -Function Paste -ErrorAction Stop
        }
    }
} catch {}

function global:Get-PebrelClipboardCapability {
    try {
        # Vi can change its active map without rendering another shell prompt.
        if ((Get-PSReadLineOption -ErrorAction Stop).EditMode -ne 'Vi' -and
            (Get-PebrelClipboardBinding).Function -eq 'Paste') {
            return $global:PebrelShellToken
        }
    } catch {}
    return ''
}
