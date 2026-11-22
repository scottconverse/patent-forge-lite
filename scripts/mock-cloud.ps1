# Loopback stand-in for the Anthropic Messages endpoint.
#
# Serves /v1/messages with a valid SSE stream and records every request, so
# acceptance can prove a declined run sent nothing and an approved run reached the
# provider -- without contacting a real host or spending anything.
param(
    [Parameter(Mandatory)] [int] $Port,
    [Parameter(Mandatory)] [string] $LogFile,
    [string] $Mode = "complete"
)
$ErrorActionPreference = 'Stop'
$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add("http://127.0.0.1:$Port/")
$listener.Start()
"ready" | Set-Content -Path "$LogFile.ready" -NoNewline

try {
    while ($true) {
        $context = $listener.GetContext()
        $reader = New-Object System.IO.StreamReader($context.Request.InputStream)
        $body = $reader.ReadToEnd()
        $reader.Dispose()
        Add-Content -Path $LogFile -Value ((Get-Date).ToUniversalTime().ToString('o') + ' ' + $body.Length)

        $response = $context.Response
        $response.StatusCode = 200
        $response.ContentType = 'text/event-stream'
        $frames = @(
            "event: message_start`ndata: {`"type`":`"message_start`",`"message`":{`"usage`":{`"input_tokens`":9}}}`n`n",
            "event: content_block_delta`ndata: {`"type`":`"content_block_delta`",`"delta`":{`"text`":`"Loopback Cloud output.`"}}`n`n",
            "event: message_delta`ndata: {`"type`":`"message_delta`",`"delta`":{`"stop_reason`":`"end_turn`"},`"usage`":{`"output_tokens`":6}}`n`n",
            "event: message_stop`ndata: {`"type`":`"message_stop`"}`n`n"
        ) -join ''
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($frames)
        $response.OutputStream.Write($bytes, 0, $bytes.Length)
        $response.OutputStream.Flush()
        $response.Close()
    }
} finally {
    $listener.Stop()
    $listener.Close()
}
