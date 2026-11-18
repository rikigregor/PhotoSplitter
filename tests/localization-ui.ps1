param()
$ErrorActionPreference='Stop'
$source=Join-Path $PSScriptRoot '..\src'
$originalLanguage=$env:PHOTOSPLITTER_LANG
try {
 foreach($locale in 'en','sk') {
  $env:PHOTOSPLITTER_LANG=$locale
  $script:catalog=Get-Content -LiteralPath (Join-Path $source 'localization.json') -Raw -Encoding UTF8|ConvertFrom-Json
  . (Join-Path $source 'localization.ps1')
  $code=Get-Content -LiteralPath (Join-Path $source 'ui.ps1') -Raw -Encoding UTF8
  . ([scriptblock]::Create($code.Replace('$window.ShowDialog()|Out-Null','')))
  if($window.Title -ne (T 'app')){throw 'Incorrect window title'}
  if($ui.Choose.Content -ne (T 'choose')){throw 'Incorrect file button'}
  if($ui.Process.Content -ne (T 'process')){throw 'Incorrect processing button'}
  if($xaml -match '@[a-zA-Z]+@'){throw 'Untranslated XAML resource token'}
  $expected=if($locale -eq 'en'){"Page 2 $([char]0xb7) photo 3"}else{$script:catalog.sk.ui.label.Replace('{0}','2').Replace('{1}','3')}
  if((F 'label' @(2,3)) -ne $expected){throw 'Incorrect photo label'}
  $progress=F 'progressPhoto' @(1,4,2,3,5)
  if($progress -match '\{\d+\}' -or !$progress.Contains('1 / 4')){throw 'Incorrect progress formatting'}
  if(!(F 'savedDialog' @('test-output')).Contains('test-output')){throw 'Incorrect save message'}
  if($locale -eq 'en' -and !([string]$ui.Choose.Content).StartsWith('Choose file')){throw 'English fallback failed'}
  Write-Output "PASS: $locale desktop resources and dynamic messages"
 }
} finally {$env:PHOTOSPLITTER_LANG=$originalLanguage}
