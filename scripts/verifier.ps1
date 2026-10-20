# Verification apres integration (docs/FIABILITE.md).
#
# Ce que fait ce script :
#   1. cargo check --release : la compilation de SHOT ne donne aucun avertissement ;
#   2. toute la suite d'essais (cargo test --release) ;
#   3. les essais de fiabilite, un par un, par leur nom : aller-retour des projets, fumee de toute
#      l'application, fichiers abimes, demarrage, travail a plusieurs ;
#   4. avec -Repetitions N : toute la suite N fois de suite, pour faire apparaitre un essai
#      intermittent (il est nomme dans le resume, avec le nombre de fois ou il a echoue).
# Puis il affiche un resume OK / ECHEC (avec le E accent aigu majuscule) et sort avec le code 0 (tout est OK)
# ou 1.
#
# Il ne lance PAS shot.exe : les lancements visuels sont faits a part, par la session principale.
# (cargo check ne produit pas d'executable : le script marche aussi pendant que shot.exe tourne.)
#
# Utilisation (PowerShell 5.1, depuis n'importe ou) :
#   .\scripts\verifier.ps1
#   .\scripts\verifier.ps1 -Repetitions 5
#   .\scripts\verifier.ps1 -TargetDir target\msvc -SansAvertissements
#
# Le script est en ASCII : PowerShell 5.1 lit mal l'UTF-8 sans BOM. Les messages sont en francais sans
# accents ; seul "ECHEC" est ecrit avec son accent, par son code (U+00C9).

param(
    # Nombre de passages de la suite complete (1 : un seul).
    [int]$Repetitions = 1,
    # Dossier de compilation (celui de la session principale).
    [string]$TargetDir = 'target\msvc',
    # Saute la verification des avertissements de compilation.
    [switch]$SansAvertissements
)

$ErrorActionPreference = 'Stop'
$ECHEC = ([string][char]0x00C9) + 'CHEC'
$racine = Split-Path -Parent $PSScriptRoot
Set-Location $racine
$temp = Join-Path $env:TEMP ('shot-verifier-' + $PID)
New-Item -ItemType Directory -Force $temp | Out-Null

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "cargo est introuvable dans le PATH." -ForegroundColor Red
    exit 1
}

$resume = New-Object System.Collections.ArrayList
$tout_ok = $true

function Noter([string]$nom, [bool]$ok, [string]$detail) {
    $etat = 'OK'
    if (-not $ok) { $etat = $ECHEC; $script:tout_ok = $false }
    [void]$script:resume.Add(('{0,-8} {1}  {2}' -f $etat, $nom, $detail))
    $couleur = 'Green'
    if (-not $ok) { $couleur = 'Red' }
    Write-Host ('{0,-8} {1}  {2}' -f $etat, $nom, $detail) -ForegroundColor $couleur
}

# Lance cargo (sortie complete dans $fichier, sans toucher aux flux d'erreur de PowerShell).
function Lancer-Cargo([string]$arguments, [string]$fichier) {
    $commande = 'cargo ' + $arguments + ' > "' + $fichier + '" 2>&1'
    & cmd.exe /c $commande
    return $LASTEXITCODE
}

# Lit le resultat d'une suite d'essais : passes, echoues, noms des echoues.
function Lire-Resultat([string]$fichier) {
    $r = @{ passes = 0; echoues = 0; noms = @(); lu = $false }
    if (-not (Test-Path $fichier)) { return $r }
    foreach ($ligne in (Get-Content $fichier -Encoding UTF8)) {
        if ($ligne -match '^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored') {
            $p = [int]$Matches[2]
            $f = [int]$Matches[3]
            if ($p + $f -gt 0) {
                $r.passes += $p
                $r.echoues += $f
                $r.lu = $true
            }
        }
        elseif ($ligne -match '^test (.+) \.\.\. FAILED$') {
            $r.noms += $Matches[1]
        }
    }
    return $r
}

$debut = Get-Date

# 1. Aucun avertissement de compilation.
if (-not $SansAvertissements) {
    Write-Host "== Compilation (cargo check --release) =="
    $f = Join-Path $temp 'check.txt'
    $code = Lancer-Cargo ('check --release --offline --target-dir ' + $TargetDir) $f
    $avertissements = @(Select-String -Path $f -Pattern '^warning' | Where-Object { $_.Line -notmatch 'generated \d+ warning' })
    if ($code -ne 0) {
        Noter 'compilation' $false ('cargo check a echoue (voir ' + $f + ')')
    }
    elseif ($avertissements.Count -gt 0) {
        Noter 'compilation' $false ([string]$avertissements.Count + ' avertissement(s) : ' + $avertissements[0].Line)
    }
    else {
        Noter 'compilation' $true 'aucun avertissement'
    }
}

# 2. Toute la suite, Repetitions fois.
$echecs_par_essai = @{}
$passages = New-Object System.Collections.ArrayList
for ($i = 1; $i -le [Math]::Max(1, $Repetitions); $i++) {
    Write-Host ('== Suite complete, passage ' + $i + ' / ' + [Math]::Max(1, $Repetitions) + ' ==')
    $f = Join-Path $temp ('suite-' + $i + '.txt')
    $t0 = Get-Date
    $code = Lancer-Cargo ('test --release --offline --target-dir ' + $TargetDir) $f
    $duree = [int]((Get-Date) - $t0).TotalSeconds
    $r = Lire-Resultat $f
    [void]$passages.Add($r)
    foreach ($n in $r.noms) {
        if ($echecs_par_essai.ContainsKey($n)) { $echecs_par_essai[$n] += 1 } else { $echecs_par_essai[$n] = 1 }
    }
    $ok = ($code -eq 0) -and $r.lu -and ($r.echoues -eq 0)
    $detail = [string]$r.passes + ' passes, ' + [string]$r.echoues + ' echoues, ' + $duree + ' s'
    if (-not $r.lu) { $detail = 'aucun resultat lu (la compilation des essais a echoue ? voir ' + $f + ')' }
    Noter ('suite complete ' + $i + '/' + [Math]::Max(1, $Repetitions)) $ok $detail
    if (-not $ok -and $r.noms.Count -gt 0) {
        foreach ($n in $r.noms) { Write-Host ('           echoue : ' + $n) -ForegroundColor Red }
    }
}
if ($Repetitions -gt 1) {
    $intermittents = @($echecs_par_essai.Keys | Where-Object { $echecs_par_essai[$_] -lt $Repetitions })
    $constants = @($echecs_par_essai.Keys | Where-Object { $echecs_par_essai[$_] -ge $Repetitions })
    if ($intermittents.Count -eq 0 -and $constants.Count -eq 0) {
        Noter 'stabilite' $true ([string]$Repetitions + ' passages, aucun essai intermittent')
    }
    else {
        foreach ($n in $intermittents) { Noter 'intermittent' $false ($n + ' : ' + $echecs_par_essai[$n] + ' echec(s) sur ' + $Repetitions) }
        foreach ($n in $constants) { Noter 'constant' $false ($n + ' : echoue a chaque passage') }
    }
}

# 3. Les essais de fiabilite, par leur nom.
$groupes = @(
    @('aller-retour des projets (3 chemins + application)', 'fiabilite_aller_retour'),
    @('fumee de toute l''application (GPU, CPU, vignettes)', 'fiabilite_fumee'),
    @('manifestes abimes', 'fiabilite_manifestes'),
    @('entrees abimees et essais a la louche', 'fiabilite_entrees'),
    @('dessins abimes (PNG, JSON, valeurs absurdes)', 'fiabilite_dessins'),
    @('fichiers annexes, sons et modeles illisibles', 'fiabilite_annexes'),
    @('dessins d''une autre taille que la feuille', 'fiabilite_raster'),
    @('demarrage (premiere image, pinceaux, dernier projet)', 'fiabilite_demarrage'),
    @('travail a plusieurs (prises, dossiers synchronises, fil de fond, barrieres)', 'cooperation'),
    @('Espace 3D (cartes, camera, rendu, format 35, anciens projets avec objets et repere 3D)', 'espace3d')
)
Write-Host "== Essais de fiabilite, un par un =="
foreach ($g in $groupes) {
    $f = Join-Path $temp ('groupe-' + $g[1] + '.txt')
    $t0 = Get-Date
    $code = Lancer-Cargo ('test --release --offline --target-dir ' + $TargetDir + ' ' + $g[1]) $f
    $duree = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
    $r = Lire-Resultat $f
    $ok = ($code -eq 0) -and $r.lu -and ($r.echoues -eq 0)
    $detail = [string]$r.passes + ' essai(s), ' + $duree + ' s'
    if (-not $r.lu) { $detail = 'aucun essai trouve pour ' + $g[1] }
    elseif ($r.echoues -gt 0) { $detail = [string]$r.echoues + ' echoue(s) : ' + ($r.noms -join ', ') }
    Noter $g[0] $ok $detail
}

# Resume.
$total = [int]((Get-Date) - $debut).TotalSeconds
Write-Host ''
Write-Host '==================== RESUME ===================='
foreach ($l in $resume) {
    $couleur = 'Green'
    if ($l -like ($ECHEC + '*')) { $couleur = 'Red' }
    Write-Host $l -ForegroundColor $couleur
}
Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
# Menage des dossiers d'essai (shot-perf-bac-<processus>-<fil>) : ceux dont le processus d'essai est fini partent ;
# ceux d'un passage qui tourne encore (un autre verifier, un autre dossier de travail) restent.
$vivants = @{}
Get-Process | ForEach-Object { $vivants[[string]$_.Id] = $true }
Get-ChildItem $env:TEMP -Directory -Filter 'shot-perf-bac-*' -ErrorAction SilentlyContinue | ForEach-Object {
    if ($_.Name -match '^shot-perf-bac-(\d+)-' -and -not $vivants.ContainsKey($Matches[1])) {
        Remove-Item -Recurse -Force $_.FullName -ErrorAction SilentlyContinue
    }
}
Write-Host ''
if ($tout_ok) {
    Write-Host ('RESULTAT : OK (' + $total + ' s)') -ForegroundColor Green
    exit 0
}
Write-Host ('RESULTAT : ' + $ECHEC + ' (' + $total + ' s)') -ForegroundColor Red
exit 1
