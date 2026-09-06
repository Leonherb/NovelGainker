$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskArtifact = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../target/qa/nr-505/epub-structure-fixture.epub'))
if ((Get-Item -LiteralPath $taskArtifact).Length -gt 112MB) { throw 'Synthetic EPUB exceeds bound' }
function Assert-Structure([bool]$Condition, [string]$Reason) {
    if (-not $Condition) { throw $Reason }
}
$taskHeader = [IO.File]::OpenRead($taskArtifact)
$taskBinary = [IO.BinaryReader]::new($taskHeader)
try {
    Assert-Structure ($taskBinary.ReadUInt32() -eq 0x04034b50) 'Missing first ZIP local header'
    $null = $taskBinary.ReadUInt16()
    Assert-Structure ($taskBinary.ReadUInt16() -eq 0) 'mimetype flags must be zero'
    Assert-Structure ($taskBinary.ReadUInt16() -eq 0) 'mimetype must be stored'
    $null = $taskBinary.ReadBytes(16)
    $taskNameLength = $taskBinary.ReadUInt16()
    Assert-Structure ($taskBinary.ReadUInt16() -eq 0) 'mimetype must have no extra field'
    Assert-Structure ([Text.Encoding]::ASCII.GetString($taskBinary.ReadBytes($taskNameLength)) -eq 'mimetype') 'First entry must be mimetype'
} finally { $taskBinary.Dispose(); $taskHeader.Dispose() }
$taskArchive = [IO.Compression.ZipFile]::OpenRead($taskArtifact)
try {
    $taskNames = @($taskArchive.Entries | ForEach-Object { $_.FullName })
    $taskExpected = @('mimetype', 'META-INF/container.xml', 'EPUB/images/a0.png', 'EPUB/c0.xhtml', 'EPUB/c1.xhtml', 'EPUB/nav.xhtml', 'EPUB/package.opf')
    Assert-Structure (($taskNames -join '|') -eq ($taskExpected -join '|')) 'ZIP whitelist/order mismatch'
    $taskXml = @{}
    foreach ($taskEntry in $taskArchive.Entries) {
        Assert-Structure ($taskEntry.Length -le 16MB) 'Entry bound exceeded'
        $taskStream = $taskEntry.Open()
        try {
            if ($taskEntry.FullName -eq 'mimetype') {
                $taskTextReader = [IO.StreamReader]::new($taskStream, [Text.UTF8Encoding]::new($false, $true))
                Assert-Structure ($taskTextReader.ReadToEnd() -ceq 'application/epub+zip') 'mimetype contents mismatch'
                $taskTextReader.Dispose()
            } elseif ($taskEntry.FullName -match '\.(xml|opf|xhtml)$') {
                $taskSettings = [Xml.XmlReaderSettings]::new()
                $taskSettings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
                $taskSettings.XmlResolver = $null
                $taskSettings.MaxCharactersInDocument = 8MB
                $taskReader = [Xml.XmlReader]::Create($taskStream, $taskSettings)
                $taskDocument = [Xml.XmlDocument]::new()
                $taskDocument.XmlResolver = $null
                try { $taskDocument.Load($taskReader) } finally { $taskReader.Dispose() }
                $taskXml[$taskEntry.FullName] = $taskDocument
            }
        } finally { $taskStream.Dispose() }
    }
    $taskOpf = $taskXml['EPUB/package.opf']
    $taskNs = [Xml.XmlNamespaceManager]::new($taskOpf.NameTable)
    $taskNs.AddNamespace('p', 'http://www.idpf.org/2007/opf')
    $taskNs.AddNamespace('dc', 'http://purl.org/dc/elements/1.1/')
    Assert-Structure ($taskOpf.DocumentElement.GetAttribute('version') -eq '3.0') 'Wrong package version'
    foreach ($taskField in @('identifier', 'title', 'creator', 'language')) {
        Assert-Structure ($taskOpf.SelectNodes("/p:package/p:metadata/dc:$taskField", $taskNs).Count -eq 1) 'Missing or duplicate metadata'
    }
    Assert-Structure ($taskOpf.SelectSingleNode('/p:package/p:metadata/dc:language', $taskNs).InnerText -eq 'zh-CN') 'Wrong language'
    $taskModified = $taskOpf.SelectNodes('/p:package/p:metadata/p:meta[@property="dcterms:modified"]', $taskNs)
    Assert-Structure ($taskModified.Count -eq 1 -and $taskModified[0].InnerText -match '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$') 'Missing UTC modified metadata'
    $taskManifest = $taskOpf.SelectNodes('/p:package/p:manifest/p:item', $taskNs)
    Assert-Structure ($taskManifest.Count -eq 4) 'Wrong manifest count'
    foreach ($taskItem in $taskManifest) {
        Assert-Structure ($taskNames -contains ('EPUB/' + $taskItem.GetAttribute('href'))) 'Manifest references missing entry'
    }
    Assert-Structure ($taskOpf.SelectNodes('/p:package/p:manifest/p:item[@properties="nav"]', $taskNs).Count -eq 1) 'Missing unique navigation'
    Assert-Structure ($taskOpf.SelectNodes('/p:package/p:manifest/p:item[@properties="cover-image"]', $taskNs).Count -eq 1) 'Missing cover property'
    $taskSpine = @($taskOpf.SelectNodes('/p:package/p:spine/p:itemref', $taskNs) | ForEach-Object { $_.GetAttribute('idref') })
    Assert-Structure (($taskSpine -join '|') -eq 'c0|c1') 'Spine chapter order mismatch'
    $taskNav = $taskXml['EPUB/nav.xhtml']
    $taskNavNs = [Xml.XmlNamespaceManager]::new($taskNav.NameTable)
    $taskNavNs.AddNamespace('h', 'http://www.w3.org/1999/xhtml')
    $taskNavNs.AddNamespace('e', 'http://www.idpf.org/2007/ops')
    Assert-Structure ($taskNav.SelectNodes('//h:nav[@e:type="toc"]', $taskNavNs).Count -eq 1) 'Missing unique toc'
    $taskLinks = @($taskNav.SelectNodes('//h:nav[@e:type="toc"]/h:ol/h:li/h:a', $taskNavNs) | ForEach-Object { $_.GetAttribute('href') })
    Assert-Structure (($taskLinks -join '|') -eq 'c0.xhtml|c1.xhtml') 'Navigation chapter order mismatch'
    foreach ($taskName in @('EPUB/c0.xhtml', 'EPUB/c1.xhtml', 'EPUB/nav.xhtml')) {
        foreach ($taskNode in $taskXml[$taskName].SelectNodes('//*[@href or @src]')) {
            foreach ($taskAttribute in @('href', 'src')) {
                if ($taskNode.HasAttribute($taskAttribute)) {
                    $taskReference = $taskNode.GetAttribute($taskAttribute)
                    Assert-Structure ($taskReference -notmatch '(^/|\.\.|:|\\)' -and $taskNames -contains ('EPUB/' + $taskReference)) 'Unsafe or missing resource reference'
                }
            }
        }
    }
    $taskHasher = [Security.Cryptography.SHA256]::Create()
    $taskHashStream = [IO.File]::OpenRead($taskArtifact)
    try { $taskHash = [BitConverter]::ToString($taskHasher.ComputeHash($taskHashStream)).Replace('-', '').ToLowerInvariant() }
    finally { $taskHashStream.Dispose(); $taskHasher.Dispose() }
    [ordered]@{ result = 'pass'; validator = 'independent-dotnet-zip-xml'; completeEpubCertification = $false; entries = $taskNames.Count; chapters = $taskSpine.Count; artifactSha256 = $taskHash } | ConvertTo-Json -Compress
} finally { $taskArchive.Dispose() }
