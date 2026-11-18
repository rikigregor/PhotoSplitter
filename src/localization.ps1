$script:language=if($env:PHOTOSPLITTER_LANG -eq 'sk'){'sk'}else{'en'}
$script:strings=$script:catalog.$script:language.ui
function T($key) {return [string]$script:strings.$key}
function F($key,[object[]]$values) {return [string]::Format([Globalization.CultureInfo]::CurrentCulture,(T $key),$values)}
