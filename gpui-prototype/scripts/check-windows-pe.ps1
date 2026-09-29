param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [ValidateSet('x86_64', 'arm64')]
    [string]$Arch
)

$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
[byte[]]$bytes = [System.IO.File]::ReadAllBytes($exe)
if ($bytes.Length -lt 512 -or [System.Text.Encoding]::ASCII.GetString($bytes, 0, 2) -ne 'MZ') {
    throw "Not a PE executable: $exe"
}
$pe = [BitConverter]::ToInt32($bytes, 0x3c)
if ($pe -lt 0 -or $pe + 264 -gt $bytes.Length -or
    [System.Text.Encoding]::ASCII.GetString($bytes, $pe, 4) -ne "PE`0`0") {
    throw "Invalid PE header: $exe"
}
$machine = [BitConverter]::ToUInt16($bytes, $pe + 4)
$expectedMachine = if ($Arch -eq 'arm64') { 0xaa64 } else { 0x8664 }
if ($machine -ne $expectedMachine) {
    throw "Incorrect PE architecture: expected $Arch (0x$($expectedMachine.ToString('x'))), got 0x$($machine.ToString('x'))"
}
$sections = [BitConverter]::ToUInt16($bytes, $pe + 6)
$optionalSize = [BitConverter]::ToUInt16($bytes, $pe + 20)
$optional = $pe + 24
if ([BitConverter]::ToUInt16($bytes, $optional) -ne 0x20b) {
    throw 'Expected a 64-bit PE32+ executable'
}
$subsystem = [BitConverter]::ToUInt16($bytes, $optional + 68)
if ($subsystem -ne 2) {
    throw "Expected Windows GUI subsystem (2), got $subsystem (console = 3)"
}

# Data directory 2 contains the resource tree. Verify an RT_GROUP_ICON
# entry (type 14), not just a generic .rsrc section or default manifest.
$resourceRva = [BitConverter]::ToUInt32($bytes, $optional + 112 + 2 * 8)
$resourceSize = [BitConverter]::ToUInt32($bytes, $optional + 112 + 2 * 8 + 4)
if ($resourceRva -eq 0 -or $resourceSize -lt 32) {
    throw 'No Windows PE resource directory'
}
$sectionTable = $optional + $optionalSize
$resourceOffset = -1
$resourceRawSize = 0
for ($index = 0; $index -lt $sections; $index++) {
    $section = $sectionTable + 40 * $index
    if ($section + 40 -gt $bytes.Length) { throw 'Truncated PE section table' }
    $virtualSize = [BitConverter]::ToUInt32($bytes, $section + 8)
    $virtualAddress = [BitConverter]::ToUInt32($bytes, $section + 12)
    $rawSize = [BitConverter]::ToUInt32($bytes, $section + 16)
    $rawOffset = [BitConverter]::ToUInt32($bytes, $section + 20)
    if ($resourceRva -ge $virtualAddress -and
        $resourceRva -lt $virtualAddress + [Math]::Max($virtualSize, $rawSize)) {
        $resourceOffset = [int]($rawOffset + $resourceRva - $virtualAddress)
        $resourceRawSize = [int]($rawSize - ($resourceRva - $virtualAddress))
        break
    }
}
if ($resourceOffset -lt 0 -or $resourceOffset + 16 -gt $bytes.Length) {
    throw 'PE resource directory cannot be mapped to the executable'
}
# A resource directory has type -> ID -> language -> data-entry levels.
# Read the icon-group header, not just the type number: a manifest renamed
# to type 14 must not be mistaken for a real Explorer icon.
function Read-ResourceDirectoryEntries([int]$Relative) {
    $directory = $resourceOffset + $Relative
    if ($Relative -lt 0 -or $Relative + 16 -gt $resourceSize -or
        $Relative + 16 -gt $resourceRawSize -or $directory + 16 -gt $bytes.Length) {
        throw 'Resource subdirectory outside PE resource section'
    }
    $named = [BitConverter]::ToUInt16($bytes, $directory + 12)
    $numbered = [BitConverter]::ToUInt16($bytes, $directory + 14)
    $end = $Relative + 16 + 8 * ($named + $numbered)
    if ($end -gt $resourceSize -or $end -gt $resourceRawSize -or
        $resourceOffset + $end -gt $bytes.Length) {
        throw 'Truncated PE resource directory'
    }
    $entries = @()
    for ($index = 0; $index -lt $named + $numbered; $index++) {
        $entry = $directory + 16 + 8 * $index
        $entries += ,@(
            [BitConverter]::ToUInt32($bytes, $entry),
            [BitConverter]::ToUInt32($bytes, $entry + 4)
        )
    }
    return ,$entries
}

$group = $null
$icon = $null
foreach ($entry in (Read-ResourceDirectoryEntries 0)) {
    if (($entry[0] -band 0x80000000) -ne 0) { continue }
    if ($entry[0] -eq 14) { $group = $entry }
    if ($entry[0] -eq 3) { $icon = $entry }
}
if ($null -eq $group -or ($group[1] -band 0x80000000) -eq 0) {
    throw 'Missing RT_GROUP_ICON (14): Explorer would show a generic EXE icon'
}
$ids = Read-ResourceDirectoryEntries ([int]($group[1] -band 0x7fffffff))
if ($ids.Count -eq 0 -or ($ids[0][1] -band 0x80000000) -eq 0) {
    throw 'Missing group icon ID directory'
}
$languages = Read-ResourceDirectoryEntries ([int]($ids[0][1] -band 0x7fffffff))
if ($languages.Count -eq 0 -or ($languages[0][1] -band 0x80000000) -ne 0) {
    throw 'Missing group icon language data entry'
}
$dataRelative = [int]$languages[0][1]
if ($dataRelative + 16 -gt $resourceSize -or $dataRelative + 16 -gt $resourceRawSize) {
    throw 'Icon resource data entry is outside PE section'
}
$dataEntry = $resourceOffset + $dataRelative
$dataRva = [BitConverter]::ToUInt32($bytes, $dataEntry)
$dataSize = [BitConverter]::ToUInt32($bytes, $dataEntry + 4)
$offsetInResource = [long]$dataRva - $resourceRva
if ($offsetInResource -lt 0 -or $offsetInResource + $dataSize -gt $resourceRawSize -or
    $dataSize -lt 20 -or $resourceOffset + $offsetInResource + $dataSize -gt $bytes.Length) {
    throw 'Invalid group icon resource data'
}
$header = [int]($resourceOffset + $offsetInResource)
$count = [BitConverter]::ToUInt16($bytes, $header + 4)
if ([BitConverter]::ToUInt16($bytes, $header) -ne 0 -or
    [BitConverter]::ToUInt16($bytes, $header + 2) -ne 1 -or
    $count -eq 0 -or 6 + 14 * $count -gt $dataSize) {
    throw 'RT_GROUP_ICON does not contain a valid icon group header'
}
$firstImageId = [BitConverter]::ToUInt16($bytes, $header + 6 + 12)
if ($null -eq $icon -or ($icon[1] -band 0x80000000) -eq 0) {
    throw 'Missing RT_ICON (3) image referenced by icon group'
}
$imageEntries = Read-ResourceDirectoryEntries ([int]($icon[1] -band 0x7fffffff))
if (-not @($imageEntries | Where-Object { $_[0] -eq $firstImageId }).Count) {
    throw "Icon group references missing RT_ICON image ID $firstImageId"
}
Write-Host "Verified Windows GUI PE: $Arch, $count embedded icon image(s), RT_ICON image $firstImageId"
