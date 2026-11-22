# Loopback stand-in for the USPTO PatentsView search endpoint.
#
# Serves /api/v1/patent/ with the response shape the adapter parses, and logs every
# request so acceptance can prove a denied run queried nothing and an opted-in run
# queried exactly once.
param(
    [Parameter(Mandatory)] [int] $Port,
    [Parameter(Mandatory)] [string] $LogFile
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
        $response.ContentType = 'application/json'
        $payload = '{"patents":[{"patent_id":"10123456","patent_title":"Container with integrated temperature indicator","patent_date":"2022-05-17","assignee_organization":"Acme Sensors"}],"total_hits":1}'
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
        $response.OutputStream.Write($bytes, 0, $bytes.Length)
        $response.OutputStream.Flush()
        $response.Close()
    }
} finally {
    $listener.Stop()
    $listener.Close()
}
