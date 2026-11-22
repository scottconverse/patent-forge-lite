# Loopback Ollama stand-in for installed acceptance.
#
# Serves /api/chat with NDJSON exactly as the application expects, and records
# every request so a resume can be shown to skip already-completed stages.
#
#   -ModeFile  one word: complete | truncated | limited
#   -LogFile   one line per request: "<utc> <stage-bytes> <mode>"
#
# This never contacts a model, never leaves loopback, and holds no data.
param(
    [Parameter(Mandatory)] [int] $Port,
    [Parameter(Mandatory)] [string] $ModeFile,
    [Parameter(Mandatory)] [string] $LogFile
)
$ErrorActionPreference = 'Stop'
$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add("http://127.0.0.1:$Port/")
$listener.Start()
"mock ready on 127.0.0.1:$Port" | Set-Content -Path "$ModeFile.ready" -NoNewline

try {
    while ($true) {
        $context = $listener.GetContext()
        $reader = New-Object System.IO.StreamReader($context.Request.InputStream)
        $body = $reader.ReadToEnd()
        $reader.Dispose()

        $mode = if (Test-Path $ModeFile) { (Get-Content $ModeFile -Raw).Trim() } else { 'complete' }
        $count = if (Test-Path $LogFile) { (Get-Content $LogFile | Measure-Object).Count + 1 } else { 1 }
        ("{0} {1} {2}" -f (Get-Date).ToUniversalTime().ToString('o'), $body.Length, $mode) |
            Add-Content -Path $LogFile
        # "truncate_after:N" completes N stages, then truncates the rest, so a
        # resume can be shown to skip stages that already finished.
        if ($mode -like 'truncate_after:*') {
            $limit = [int]($mode.Split(':')[1])
            $mode = if ($count -le $limit) { 'complete' } else { 'truncated' }
        }

        # The model list endpoint, so the picker can be exercised without a runtime.
        if ($context.Request.Url.AbsolutePath -like '*/api/tags') {
            $tags = $context.Response
            $tags.StatusCode = 200
            $tags.ContentType = 'application/json'
            $payload = '{"models":[{"name":"mock-model:latest"},{"name":"mock-small:7b"}]}'
            $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
            $tags.OutputStream.Write($bytes, 0, $bytes.Length)
            $tags.OutputStream.Flush()
            $tags.Close()
            continue
        }

        $response = $context.Response
        $response.StatusCode = 200
        $response.ContentType = 'application/x-ndjson'

        $text = "Loopback mock stage output for installed acceptance."
        if ($mode -eq 'truncated') {
            # Content, then the connection ends with no terminal frame.
            $stream = [System.Text.Encoding]::UTF8.GetBytes(
                '{"message":{"content":"' + $text + '"},"done":false}' + "`n")
        } elseif ($mode -eq 'limited') {
            # A terminal frame that reports an output limit rather than a stop.
            $stream = [System.Text.Encoding]::UTF8.GetBytes(
                '{"message":{"content":"' + $text + '"},"done":false}' + "`n" +
                '{"message":{"content":""},"done":true,"done_reason":"length","prompt_eval_count":7,"eval_count":4096}' + "`n")
        } else {
            $stream = [System.Text.Encoding]::UTF8.GetBytes(
                '{"message":{"content":"' + $text + '"},"done":false}' + "`n" +
                '{"message":{"content":" Mock."},"done":false}' + "`n" +
                '{"message":{"content":""},"done":true,"done_reason":"stop","prompt_eval_count":7,"eval_count":11}' + "`n")
        }

        $response.OutputStream.Write($stream, 0, $stream.Length)
        $response.OutputStream.Flush()
        $response.Close()
    }
} finally {
    $listener.Stop()
    $listener.Close()
}
