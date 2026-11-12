param(
    [int]$IntervalSeconds = 10,
    [int]$ExpectedNodes = 8,
    [string]$Enclave = 'eva-guard-net',
    [int]$RpcTimeoutSec = 3,
    [int]$BeaconTimeoutSec = 3,
    [int]$ErrorWindowRounds = 20
)

$ErrorActionPreference = 'Stop'
if ($IntervalSeconds -lt 1 -or $ExpectedNodes -lt 1 -or $RpcTimeoutSec -lt 1 -or $BeaconTimeoutSec -lt 1 -or $ErrorWindowRounds -lt 1) {
    throw 'IntervalSeconds, ExpectedNodes, RpcTimeoutSec, BeaconTimeoutSec and ErrorWindowRounds must all be >= 1.'
}

function Convert-SizeToBytes {
    param([string]$SizeText)
    $matchSize = [regex]::Match($SizeText.Trim(), '^([0-9]+(?:\.[0-9]+)?)\s*([kmgt]?i?b)$', 'IgnoreCase')
    if (-not $matchSize.Success) { throw ('Cannot parse Docker size: {0}' -f $SizeText) }
    $value = [double]::Parse($matchSize.Groups[1].Value, [System.Globalization.CultureInfo]::InvariantCulture)
    $unit = $matchSize.Groups[2].Value.ToLowerInvariant()
    $multiplier = switch ($unit) {
        'b' { 1 } 'kb' { 1000 } 'mb' { 1000000 } 'gb' { 1000000000 }
        'tb' { 1000000000000 } 'kib' { 1024 } 'mib' { 1048576 }
        'gib' { 1073741824 } 'tib' { 1099511627776 }
        default { throw ('Unsupported Docker size unit: {0}' -f $unit) }
    }
    return ($value * $multiplier)
}

function Convert-HexNumber {
    param([object]$HexValue)
    if ([string]$HexValue -notmatch '^0x[0-9a-fA-F]+$') {
        throw ('Invalid Ethereum hex number: {0}' -f $HexValue)
    }
    return [Convert]::ToInt64(([string]$HexValue).Substring(2), 16)
}

function Invoke-GethRpcMeasured {
    param([string]$Url, [string]$Method, [int]$TimeoutSec)
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $body = @{ jsonrpc = '2.0'; method = $Method; params = @(); id = 1 } | ConvertTo-Json -Compress
        $response = Invoke-RestMethod -Uri $Url -Method Post -ContentType 'application/json' -Body $body -TimeoutSec $TimeoutSec
        if ($null -ne $response.error) {
            throw ('JSON-RPC error ({0}): {1}' -f $response.error.code, $response.error.message)
        }
        if ($null -eq $response -or -not ($response.PSObject.Properties.Name -contains 'result')) {
            throw 'JSON-RPC response has no result property.'
        }
        $watch.Stop()
        # Explicit -NoEnumerate semantics: preserve arrays (e.g. admin_peers), and bool false.
        return [PSCustomObject]@{ ok = $true; result = $response.result; latency_ms = [math]::Round($watch.Elapsed.TotalMilliseconds, 2); error = $null; timeout = $false }
    }
    catch {
        $watch.Stop()
        $message = [string]$_.Exception.Message
        $isTimeout = ($message -match '(?i)timed?\s*out|timeout|operation has timed out')
        return [PSCustomObject]@{ ok = $false; result = $null; latency_ms = [math]::Round($watch.Elapsed.TotalMilliseconds, 2); error = $message; timeout = $isTimeout }
    }
}

function Invoke-BeaconGet {
    param([string]$BaseUrl, [string]$Path, [int]$TimeoutSec)
    try {
        $response = Invoke-RestMethod -Uri ($BaseUrl + $Path) -Method Get -TimeoutSec $TimeoutSec
        if ($null -eq $response -or $null -eq $response.data) { throw 'Beacon response is missing data.' }
        return [PSCustomObject]@{ ok = $true; data = $response.data; error = $null }
    }
    catch {
        return [PSCustomObject]@{ ok = $false; data = $null; error = [string]$_.Exception.Message }
    }
}

# Match exactly the current Kurtosis service and the published port, not a fixed port.
# Works for service name on one line and its port on the same or following lines.
Write-Host 'EVA-Guard v4: discovering Geth and Lodestar endpoints...' -ForegroundColor Cyan
$inspection = @(wsl.exe -d Ubuntu -- kurtosis enclave inspect $Enclave)
if ($LASTEXITCODE -ne 0) { throw ('Kurtosis enclave inspect failed for {0}.' -f $Enclave) }

$urls = @{}
$beaconUrls = @{}
$currentService = $null
$currentNode = $null
foreach ($line in $inspection) {
    if ($line -match '\b(el|cl)-(\d+)-(geth-lodestar|lodestar-geth)\b') {
        $role = [string]$Matches[1]
        $currentNode = [int]$Matches[2]
        $clientPair = [string]$Matches[3]
        $currentService = $null
        if ($role -eq 'el' -and $clientPair -eq 'geth-lodestar') { $currentService = 'el' }
        if ($role -eq 'cl' -and $clientPair -eq 'lodestar-geth') { $currentService = 'cl' }
    }
    elseif ($line -match '\b(?:el|cl|vc)-\d+-[a-z0-9-]+\b' -and $line -match '^\s*[a-f0-9-]{6,}\s+') {
        # Prevent accidentally attributing ports to a previous service.
        $currentService = $null
        $currentNode = $null
    }

    if ($null -ne $currentNode -and $currentService -eq 'el' -and
        $line -match '\brpc:\s*8545/tcp\s*->\s*(?:https?://)?(?:127\.0\.0\.1|localhost):(\d+)') {
        $urls[$currentNode] = ('http://127.0.0.1:{0}' -f $Matches[1])
    }
    if ($null -ne $currentNode -and $currentService -eq 'cl' -and
        $line -match '\b(?:http|rest|beacon|beacon-api):\s*\d+/tcp\s*->\s*(?:https?://)?(?:127\.0\.0\.1|localhost):(\d+)') {
        $beaconUrls[$currentNode] = ('http://127.0.0.1:{0}' -f $Matches[1])
    }
}
if ($urls.Count -ne $ExpectedNodes) {
    throw ('Found {0} Geth RPC endpoints, expected {1}. Check Kurtosis enclave inspect.' -f $urls.Count, $ExpectedNodes)
}
if ($beaconUrls.Count -ne $ExpectedNodes) {
    Write-Warning ('Found {0}/{1} Lodestar REST endpoints. Missing CL metrics will remain blank, not zero.' -f $beaconUrls.Count, $ExpectedNodes)
}

Write-Host 'Discovering matching Geth Docker containers...'
$dockerLines = @(docker ps --format '{{json .}}')
if ($LASTEXITCODE -ne 0) { throw 'docker ps failed.' }
$containers = @{}
foreach ($line in $dockerLines) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $item = $line | ConvertFrom-Json
    if ($item.Names -match '^el-(\d+)-geth-lodestar(?:--|$)') {
        $containers[[int]$Matches[1]] = [string]$item.Names
    }
}
if ($containers.Count -ne $ExpectedNodes) {
    Write-Warning ('Found {0}/{1} Geth Docker containers. Missing system metrics will be blank.' -f $containers.Count, $ExpectedNodes)
}

$dataDirectory = Join-Path (Split-Path -Parent $PSScriptRoot) 'data'
New-Item -ItemType Directory -Path $dataDirectory -Force | Out-Null
$runId = Get-Date -Format 'yyyyMMdd_HHmmss'
$outputPath = Join-Path $dataDirectory ('ethereum_full_baseline_{0}.csv' -f $runId)
$previous = @{}
$errorHistory = @{}
Write-Host ('Collecting {0} Geth nodes and {1} Lodestar endpoints into {2}' -f $urls.Count, $beaconUrls.Count, $outputPath) -ForegroundColor Green
Write-Host ('Sampling every ~{0}s after each sweep (sweeps can take longer during RPC timeouts). Ctrl+C stops.' -f $IntervalSeconds)
Write-Host 'RPC error rate is percent of failed core probes over the last specified rounds; the first sample has no rate/churn yet.'

while ($true) {
    $statsByName = @{}
    try {
        $statsLines = @(docker stats --no-stream --format '{{json .}}')
        if ($LASTEXITCODE -ne 0) { throw 'docker stats failed.' }
        foreach ($line in $statsLines) {
            if ([string]::IsNullOrWhiteSpace($line)) { continue }
            $stat = $line | ConvertFrom-Json
            $statsByName[[string]$stat.Name] = $stat
        }
    }
    catch { Write-Warning ('Docker stats unavailable for this sweep: {0}' -f $_.Exception.Message) }

    $sweep = New-Object 'System.Collections.Generic.List[object]'
    foreach ($node in ($urls.Keys | Sort-Object)) {
         $now = (Get-Date).ToUniversalTime()
        $url = $urls[$node]
        $record = [ordered]@{
            timestamp_utc = $now.ToString('o')
            run_id = $runId
            node_id = ('node{0}' -f $node)
            total_nodes = $ExpectedNodes
            peer_count = $null
            peer_ids = ''
            peer_additions = $null
            peer_drops = $null
            block_number = $null
            block_delta = $null
            seconds_since_block_change = $null
            syncing = $null
            cpu_percent = $null
            memory_mb = $null
            rx_total_bytes_approx = $null
            tx_total_bytes_approx = $null
            rx_bytes_per_sec_approx = $null
            tx_bytes_per_sec_approx = $null
            scenario = 'normal_candidate'
            # New: EL RPC metrics (a round probes exactly 3 core methods).
            rpc_latency_ms = $null
            rpc_request_count = 0
            rpc_error_count = 0
            rpc_timeout_count = 0
            rpc_window_requests = 0
            rpc_window_errors = 0
            rpc_error_rate = $null
            # New: calculated AFTER collecting all 8 nodes in a sweep.
            network_reference_block = $null
 sync_lag_blocks = $null
            # New: Lodestar consensus REST API.
            consensus_peer_count = $null
            consensus_head_slot = $null
            consensus_sync_distance = $null
            consensus_is_syncing = $null
            consensus_is_optimistic = $null
            consensus_el_offline = $null
            consensus_api_ok = $false
            consensus_api_error = ''
            # Extra observability: missing is not mistaken for a zero.
            geth_rpc_ok = $false
            geth_rpc_error = ''
            peer_info_available = $false
            docker_stats_ok = $false
        }
        $old = $null
        if ($previous.ContainsKey($node)) { $old = $previous[$node] }

        # Each core RPC call has its own failure isolation: no dropped CSV row.
        $methods = @('net_peerCount','eth_blockNumber','eth_syncing')
        $responses = @{}
        $errors = New-Object 'System.Collections.Generic.List[string]'
        $roundFailures = 0
        $roundTimeouts = 0
        foreach ($method in $methods) {
            $call = Invoke-GethRpcMeasured -Url $url -Method $method -TimeoutSec $RpcTimeoutSec
            $responses[$method] = $call
            $record.rpc_request_count++
            if (-not $call.ok) {
                $roundFailures++
                if ($call.timeout) { $roundTimeouts++ }
                $errors.Add(('{0}: {1}' -f $method, $call.error))
            }
        }
        $record.rpc_error_count = $roundFailures
        $record.rpc_timeout_count = $roundTimeouts
        $record.geth_rpc_ok = ($roundFailures -eq 0)
        $record.geth_rpc_error = ($errors -join ' | ')
        if ($responses['eth_blockNumber'].ok) {
            # Always measure the SAME method for a comparable latency timeseries.
            $record.rpc_latency_ms = $responses['eth_blockNumber'].latency_ms
            try { $record.block_number = Convert-HexNumber $responses['eth_blockNumber'].result }
            catch { $record.geth_rpc_ok = $false; $record.geth_rpc_error += (' | invalid block number: ' + $_.Exception.Message) }
        }
        if ($responses['net_peerCount'].ok) {
            try { $record.peer_count = Convert-HexNumber $responses['net_peerCount'].result }
            catch { $record.geth_rpc_ok = $false; $record.geth_rpc_error += (' | invalid peer count: ' + $_.Exception.Message) }
        }
        if ($responses['eth_syncing'].ok) {
            $syncValue = $responses['eth_syncing'].result
            $record.syncing = -not (($syncValue -is [bool]) -and ($syncValue -eq $false))
        }

        # Rolling errors over N rounds * 3 probes each, not a single unstable ratio.
        if (-not $errorHistory.ContainsKey($node)) {
            $errorHistory[$node] = New-Object System.Collections.ArrayList
        }
        [void]$errorHistory[$node].Add([PSCustomObject]@{ requests = $record.rpc_request_count; errors = $roundFailures })
        while ($errorHistory[$node].Count -gt $ErrorWindowRounds) { $errorHistory[$node].RemoveAt(0) }
        $record.rpc_window_requests = [int](($errorHistory[$node] | Measure-Object -Property requests -Sum).Sum)
        $record.rpc_window_errors = [int](($errorHistory[$node] | Measure-Object -Property errors -Sum).Sum)
        if ($record.rpc_window_requests -gt 0) {
            $record.rpc_error_rate = [math]::Round(100.0 * $record.rpc_window_errors / $record.rpc_window_requests, 2)
        }

        # admin_peers is optional and NOT counted towards core RPC errors:
        # in some Geth deployments, the admin namespace is intentionally disabled.
        $peerIds = @()
        $peerInfoAvailable = $false
        $peerCall = Invoke-GethRpcMeasured -Url $url -Method 'admin_peers' -TimeoutSec $RpcTimeoutSec
        if ($peerCall.ok) {
            $peerIds = @($peerCall.result | Where-Object { $null -ne $_ -and $null -ne $_.id } | ForEach-Object { [string]$_.id } | Sort-Object -Unique)
            $peerInfoAvailable = $true
            $record.peer_ids = ($peerIds -join ';')
            $record.peer_info_available = $true
        }
        if ($null -ne $old -and $peerInfoAvailable -and $old.peerInfoAvailable) {
            $record.peer_additions = @($peerIds | Where-Object { $_ -notin $old.peerIds }).Count
            $record.peer_drops = @($old.peerIds | Where-Object { $_ -notin $peerIds }).Count
        }
        
        # Block progress: retain previous successful sample across RPC failures.
        $blockChangedAt = $now
        if ($null -ne $old -and $null -ne $old.blockChangedAt) { $blockChangedAt = $old.blockChangedAt }
        if ($null -ne $record.block_number) {
            if ($null -ne $old -and $null -ne $old.blockNumber) {
                $record.block_delta = $record.block_number - $old.blockNumber
                if ($record.block_number -ne $old.blockNumber) { $blockChangedAt = $now }
                $record.seconds_since_block_change = [math]::Round(($now - $blockChangedAt).TotalSeconds, 1)
            }
            else { $blockChangedAt = $now }
        }

        # System counters: do not stop the sweep when Docker is temporarily unavailable.
        $rxTotal = $null
        $txTotal = $null
        $containerName = $containers[$node]
        if ($null -ne $containerName -and $statsByName.ContainsKey($containerName)) {
            try {
                $stat = $statsByName[$containerName]
                $cpuText = ([string]$stat.CPUPerc).Replace('%', '').Trim()
                $record.cpu_percent = [math]::Round([double]::Parse($cpuText, [System.Globalization.CultureInfo]::InvariantCulture), 2)
                $memoryPart = (([string]$stat.MemUsage) -split '\s*/\s*')[0]
                $record.memory_mb = [math]::Round((Convert-SizeToBytes $memoryPart) / 1000000, 2)
                $netParts = ([string]$stat.NetIO) -split '\s*/\s*'
                if ($netParts.Count -ne 2) { throw ('Bad Docker NetIO value: {0}' -f $stat.NetIO) }
                $rxTotal = Convert-SizeToBytes $netParts[0]
                $txTotal = Convert-SizeToBytes $netParts[1]
                $record.rx_total_bytes_approx = [math]::Round($rxTotal)
                $record.tx_total_bytes_approx = [math]::Round($txTotal)
                $record.docker_stats_ok = $true
            }
            catch { Write-Warning ('Node {0} Docker stats parsing: {1}' -f $node, $_.Exception.Message) }
        }
        if ($null -ne $old -and $null -ne $rxTotal -and $null -ne $txTotal -and
            $null -ne $old.rx -and $null -ne $old.tx -and $null -ne $old.rxTime) {
            $elapsed = ($now - $old.rxTime).TotalSeconds
            if ($elapsed -gt 0 -and $rxTotal -ge $old.rx -and $txTotal -ge $old.tx) {
                $record.rx_bytes_per_sec_approx = [math]::Round(($rxTotal - $old.rx) / $elapsed, 1)
                $record.tx_bytes_per_sec_approx = [math]::Round(($txTotal - $old.tx) / $elapsed, 1)
            }
        }

        # Beacon REST API; these two independent probes are read-only.
        if ($beaconUrls.ContainsKey($node)) {
            $base = $beaconUrls[$node]
            $sync = Invoke-BeaconGet -BaseUrl $base -Path '/eth/v1/node/syncing' -TimeoutSec $BeaconTimeoutSec
            $peers = Invoke-BeaconGet -BaseUrl $base -Path '/eth/v1/node/peer_count' -TimeoutSec $BeaconTimeoutSec
            $beaconErrors = New-Object 'System.Collections.Generic.List[string]'
            if ($sync.ok) {
                try {
                    $record.consensus_head_slot = [long]::Parse([string]$sync.data.head_slot)
                    $record.consensus_sync_distance = [long]::Parse([string]$sync.data.sync_distance)
                    $record.consensus_is_syncing = [bool]$sync.data.is_syncing
                    if ($null -ne $sync.data.is_optimistic) { $record.consensus_is_optimistic = [bool]$sync.data.is_optimistic }
                    if ($null -ne $sync.data.el_offline) { $record.consensus_el_offline = [bool]$sync.data.el_offline }
                }
                catch { $beaconErrors.Add(('syncing parse: ' + $_.Exception.Message)) }
            }
            else { $beaconErrors.Add(('syncing: ' + $sync.error)) }
            if ($peers.ok) {
                try { $record.consensus_peer_count = [long]::Parse([string]$peers.data.connected) }
                catch { $beaconErrors.Add(('peer_count parse: ' + $_.Exception.Message)) }
            }
            else { $beaconErrors.Add(('peer_count: ' + $peers.error)) }
            $record.consensus_api_ok = ($beaconErrors.Count -eq 0)
            $record.consensus_api_error = ($beaconErrors -join ' | ')
        }
        else { $record.consensus_api_error = 'Lodestar REST endpoint not found in Kurtosis inspection' }

        # Store last valid counters for calculating deltas. Keep previous values if a probe failed.
        $previous[$node] = @{
            sampleTime = $now
            rxTime = $(if ($null -ne $rxTotal -and $null -ne $txTotal) { $now } elseif ($null -ne $old) { $old.rxTime } else { $null })
            rx = $(if ($null -ne $rxTotal) { $rxTotal } elseif ($null -ne $old) { $old.rx } else { $null })
            tx = $(if ($null -ne $txTotal) { $txTotal } elseif ($null -ne $old) { $old.tx } else { $null })
            peerIds = $peerIds
            peerInfoAvailable = $peerInfoAvailable
            blockNumber = $(if ($null -ne $record.block_number) { $record.block_number } elseif ($null -ne $old) { $old.blockNumber } else { $null })
            blockChangedAt = $blockChangedAt
        }
        $sweep.Add([PSCustomObject]$record)
    }# Robust local reference: upper median block height of all valid EL nodes this sweep.
    # This resists a minority of lagging/advanced outliers in a controlled 8-node network.
    # Note: NOT a trusted external reference or a proof of canonical consensus.
    $knownBlocks = @($sweep | Where-Object { $null -ne $_.block_number } | ForEach-Object { [long]$_.block_number } | Sort-Object)
    $reference = $null
    if ($knownBlocks.Count -ge 2) { $reference = $knownBlocks[[int][math]::Floor($knownBlocks.Count / 2)] }

    foreach ($entry in $sweep) {
        if ($null -ne $reference) {
            $entry.network_reference_block = $reference
            if ($null -ne $entry.block_number) {
                $lag = [long]($reference - $entry.block_number)
                if ($lag -lt 0) { $lag = 0 }
                $entry.sync_lag_blocks = $lag
            }
        }
        # Append the same schema to one CSV file for this run, even when some probes fail.
        $entry | Export-Csv -Path $outputPath -NoTypeInformation -Append -Encoding UTF8
        Write-Host ('{0} | EL peers={1} block={2} lag={3} | CPU={4}% RAM={5}MB | RPC={6}ms err={7}% | CL peers={8} slotlag={9}' -f `
            $entry.node_id, $entry.peer_count, $entry.block_number, $entry.sync_lag_blocks, $entry.cpu_percent, $entry.memory_mb,
            $entry.rpc_latency_ms, $entry.rpc_error_rate, $entry.consensus_peer_count, $entry.consensus_sync_distance)
    }
    Start-Sleep -Seconds $IntervalSeconds
}
