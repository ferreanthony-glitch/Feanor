# Lance le serveur local de generation sonore de SHOT (PowerShell 5.1).
#
#   .\sidecar\lancer.ps1
#   .\sidecar\lancer.ps1 -Racine D:\SHOT-modeles -Port 8765
#
# -Racine : le dossier SHOT-modeles (python, venv et modeles). Par defaut : la variable SHOT_MODELES,
#           sinon Documents\SHOT\Modeles (avec l'accent grave)\SHOT-modeles, sinon l'ancien
#           Documents\Work\SHOT-modeles s'il est le seul des deux a exister (comme SHOT).
# Le serveur n'ecoute que sur 127.0.0.1. Ctrl+C l'arrete. Aucun acces au reseau.
# (Le script est en ASCII : PowerShell 5.1 lit mal l'UTF-8 sans BOM.)

param(
    [string]$Racine = '',
    [int]$Port = 8765
)

$ErrorActionPreference = 'Stop'
if (-not $Racine) { $Racine = $env:SHOT_MODELES }
if (-not $Racine) {
    $documents = [Environment]::GetFolderPath('MyDocuments')
    # (Le nom du dossier a un accent : ecrit par son code, le script reste en ASCII.)
    $neuve = Join-Path $documents ('SHOT\Mod' + [char]0x00E8 + 'les\SHOT-modeles')
    $ancienne = Join-Path $documents 'Work\SHOT-modeles'
    if ((-not (Test-Path $neuve)) -and (Test-Path $ancienne)) { $Racine = $ancienne } else { $Racine = $neuve }
}
$python = Join-Path $Racine 'venv\Scripts\python.exe'
if (-not (Test-Path $python)) {
    Write-Host "Python introuvable : $python" -ForegroundColor Red
    Write-Host "Donne le dossier SHOT-modeles avec -Racine, ou la variable d'environnement SHOT_MODELES." -ForegroundColor Red
    exit 1
}
$serveur = Join-Path $PSScriptRoot 'serveur.py'
$env:HF_HUB_OFFLINE = '1'
$env:TRANSFORMERS_OFFLINE = '1'
Write-Host "Serveur local SHOT : $python" -ForegroundColor Cyan
& $python -I $serveur --racine $Racine --port $Port
exit $LASTEXITCODE
