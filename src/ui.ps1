$ErrorActionPreference='Stop'
Add-Type -AssemblyName PresentationFramework,PresentationCore,WindowsBase,System.Windows.Forms,System.Drawing
$xaml=@'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" Title="@app@" Width="1150" Height="800" MinWidth="850" MinHeight="620" WindowStartupLocation="CenterScreen" Background="#F3F5F7" FontFamily="Segoe UI" FontSize="14">
<Window.Resources><Style TargetType="Button"><Setter Property="Padding" Value="18,10"/><Setter Property="Margin" Value="0,0,10,0"/><Setter Property="Background" Value="White"/><Setter Property="BorderBrush" Value="#D7DEE5"/></Style></Window.Resources>
<Grid Margin="28" Background="#F3F5F7"><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition Height="*"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>
<StackPanel Margin="0,0,0,24"><TextBlock Text="@heading@" FontSize="30" FontWeight="SemiBold" Foreground="#20362F"/><TextBlock Text="@intro@" Margin="0,8,0,0" Foreground="#60717E"/></StackPanel>
<Border Grid.Row="1" Background="White" CornerRadius="12" Padding="18" Margin="0,0,0,20"><DockPanel><Button Name="Choose" Content="@choose@" DockPanel.Dock="Right"/><Button Name="Process" Content="@process@" DockPanel.Dock="Right" Background="#24634F" Foreground="White" IsEnabled="False"/><TextBlock Name="FileLabel" Text="@noFile@" VerticalAlignment="Center" TextTrimming="CharacterEllipsis"/></DockPanel></Border>
<Grid Grid.Row="2"><Grid.ColumnDefinitions><ColumnDefinition Width="5*"/><ColumnDefinition Width="16"/><ColumnDefinition Width="6*"/></Grid.ColumnDefinitions><Border Background="White" CornerRadius="12" Padding="16"><DockPanel><TextBlock Name="Count" Text="@gallery@" FontSize="18" FontWeight="SemiBold" DockPanel.Dock="Top" Margin="0,0,0,12"/><ListBox Name="Photos" BorderThickness="0" ScrollViewer.HorizontalScrollBarVisibility="Disabled"><ListBox.ItemsPanel><ItemsPanelTemplate><WrapPanel/></ItemsPanelTemplate></ListBox.ItemsPanel></ListBox></DockPanel></Border>
<Border Grid.Column="2" Background="White" CornerRadius="12" Padding="20"><DockPanel><TextBlock Name="SelectedLabel" Text="@preview@" FontSize="18" FontWeight="SemiBold" DockPanel.Dock="Top"/><StackPanel DockPanel.Dock="Bottom"><StackPanel Orientation="Horizontal" HorizontalAlignment="Center" Margin="0,15,0,12"><Button Name="Left" Content="@left@" IsEnabled="False"/><Button Name="Right" Content="@right@" IsEnabled="False"/></StackPanel><TextBlock Text="@original@" Foreground="#60717E" TextWrapping="Wrap"/></StackPanel><Image Name="Preview" Stretch="Uniform" Margin="0,15"/></DockPanel></Border></Grid>
<Grid Grid.Row="3" Margin="0,20,0,0"><Grid.ColumnDefinitions><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><StackPanel VerticalAlignment="Center"><TextBlock Name="Status" Text="@ready@" TextWrapping="Wrap" Foreground="#506572"/><ProgressBar Name="Progress" Height="4" Margin="0,8,20,0" Visibility="Collapsed" IsIndeterminate="True"/></StackPanel><StackPanel Grid.Column="1" Orientation="Horizontal"><Button Name="Advanced" Content="@advanced@" IsEnabled="False"/><Button Name="Save" Content="@save@" Background="#24634F" Foreground="White" IsEnabled="False"/></StackPanel></Grid>
</Grid></Window>
'@
foreach($key in $script:strings.PSObject.Properties.Name){$xaml=$xaml.Replace('@'+$key+'@',[Security.SecurityElement]::Escape((T $key)))}
$window=[Windows.Markup.XamlReader]::Load((New-Object System.Xml.XmlNodeReader ([xml]$xaml)))
$ui=@{};'Choose','Process','FileLabel','Count','Photos','SelectedLabel','Preview','Left','Right','Status','Progress','Advanced','Save'|ForEach-Object{$ui[$_]=$window.FindName($_)}
$script:items=@();$script:inputPath=$null;$script:job=$null;$script:dirty=$false
$script:tempRoot=Join-Path ([IO.Path]::GetTempPath()) ('photo-splitter-ui-'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($tempRoot)|Out-Null
function Fail($s){[Windows.MessageBox]::Show($window,$s,(T 'app'),'OK','Error')|Out-Null}
function Bitmap($path,$rotation,$width){
 $b=New-Object Windows.Media.Imaging.BitmapImage;$b.BeginInit();$b.CacheOption='OnLoad';$b.DecodePixelWidth=$width;$b.UriSource=[Uri]::new($path);$b.EndInit();$b.Freeze()
 if($rotation){$t=[Windows.Media.Imaging.TransformedBitmap]::new($b,[Windows.Media.RotateTransform]::new($rotation));$t.Freeze();return $t};return $b
}
function Selected {if($ui.Photos.SelectedIndex -lt 0){return};$item=$script:items[$ui.Photos.SelectedIndex];$ui.Preview.Source=Bitmap $item.Path $item.Rotation 1400;$ui.SelectedLabel.Text=$item.Label;$ui.Left.IsEnabled=$true;$ui.Right.IsEnabled=$true}
function Append-Photo($item) {
 $panel=New-Object Windows.Controls.StackPanel;$panel.Width=174;$panel.Margin='0,0,8,14'
 $im=New-Object Windows.Controls.Image;$im.Width=164;$im.Height=120;$im.Stretch='Uniform';$im.Source=Bitmap $item.Path $item.Rotation 320;$item.Thumb=$im
 $text=New-Object Windows.Controls.TextBlock;$text.Text=$item.Label;$text.Margin='4,7,0,0';$text.FontSize=12
 $panel.Children.Add($im)|Out-Null;$panel.Children.Add($text)|Out-Null;$ui.Photos.Items.Add($panel)|Out-Null
}
function Fill {
 $ui.Photos.Items.Clear();$ui.Preview.Source=$null
 foreach($item in $script:items){Append-Photo $item}
 $ui.Count.Text=(F 'count' @($script:items.Count));if($script:items.Count){$ui.Photos.SelectedIndex=0}
}
function Read-SharedJson($path) {
 $stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
 $reader=[IO.StreamReader]::new($stream,[Text.Encoding]::UTF8,$true)
 try{return ($reader.ReadToEnd()|ConvertFrom-Json)}finally{$reader.Dispose()}
}
function Refresh-Ready {
 if(!(Test-Path -LiteralPath $script:runDir)){return}
 foreach($file in @(Get-ChildItem -LiteralPath $script:runDir -Filter '*.ready.json'|Sort-Object Name)) {
  if($script:seen.ContainsKey($file.Name)){continue}
  try {
   $meta=Read-SharedJson $file.FullName
   $label=(F 'label' @($meta.page,$meta.photo));if($meta.uncertain){$label=(T 'check')+$label}
   $item=[pscustomobject]@{Path=(Join-Path $script:runDir $meta.name);Name=$meta.name;Label=$label;Rotation=0;Thumb=$null}
   Append-Photo $item
   $script:items+=@($item);$script:seen[$file.Name]=$true;$script:dirty=$true
   if($ui.Photos.SelectedIndex -lt 0){$ui.Photos.SelectedIndex=0}
  }catch{continue}
 }
 $ui.Count.Text=(F 'count' @($script:items.Count))
}
$ui.Choose.Add_Click({try{
 if($script:dirty -and [Windows.MessageBox]::Show($window,(T 'unsavedFile'),(T 'unsaved'),'YesNo','Question') -ne 'Yes'){return}
 $d=New-Object Windows.Forms.OpenFileDialog;$d.Title=(T 'choose');$d.Filter=(T 'filter');if($d.ShowDialog() -ne 'OK'){return};$script:inputPath=$d.FileName;$script:items=@();$script:dirty=$false;$ui.Photos.Items.Clear();$ui.Preview.Source=$null;$ui.Count.Text=(T 'gallery');foreach($n in 'Save','Advanced','Left','Right'){$ui[$n].IsEnabled=$false};$ui.FileLabel.Text=$d.FileName;$ui.Process.IsEnabled=$true;$ui.Status.Text=(T 'clickProcess')
}catch{Fail $_.Exception.Message}})
$timer=New-Object Windows.Threading.DispatcherTimer;$timer.Interval=[TimeSpan]::FromMilliseconds(300)
$ui.Process.Add_Click({try{
 if($script:dirty -and [Windows.MessageBox]::Show($window,(T 'restart'),(T 'unsaved'),'YesNo','Question') -ne 'Yes'){return}
 $script:runDir=Join-Path $tempRoot ([guid]::NewGuid().ToString('N'));$s=New-Object Diagnostics.ProcessStartInfo;$s.FileName=$env:FOTO_EXE;$s.Arguments='"'+$script:inputPath+'" --prepare-ui --out "'+$script:runDir+'"';$s.UseShellExecute=$false;$s.CreateNoWindow=$true;$s.RedirectStandardError=$true;$s.RedirectStandardOutput=$true
 $script:job=[Diagnostics.Process]::Start($s);$script:stdOut=$script:job.StandardOutput.ReadToEndAsync();$script:stdErr=$script:job.StandardError.ReadToEndAsync()
 foreach($n in 'Choose','Process','Save','Advanced','Left','Right'){$ui[$n].IsEnabled=$false};$script:items=@();$script:seen=@{};$script:dirty=$false;Fill;$ui.Photos.IsEnabled=$true;$ui.Progress.Visibility='Visible';$ui.Status.Text=(T 'working');$timer.Start()
}catch{Fail $_.Exception.Message}})
$timer.Add_Tick({
 if(!$script:job){return}
 Refresh-Ready
 if(!$script:job.HasExited){
  try{
   $progress=Read-SharedJson (Join-Path $script:runDir 'progress.json')
   if($progress.stage -eq 'photo'){$ui.Status.Text=(F 'progressPhoto' @($progress.page,$progress.pages,$progress.photo,$progress.photos,$script:items.Count))}
   else{$ui.Status.Text=(F 'progressPage' @($progress.page,$progress.pages,$script:items.Count))}
  }catch{}
  return
 }
 $timer.Stop()
 try {
  Refresh-Ready
  if($script:job.ExitCode -ne 0){throw $script:stdErr.Result}
  $ui.Advanced.IsEnabled=Test-Path -LiteralPath (Join-Path $script:runDir 'kontrola.html')
  $ui.Status.Text=(F 'complete' @($script:items.Count))
  if(!$script:items.Count){$ui.Status.Text=(T 'empty')}
 }catch{Fail $_.Exception.Message;$ui.Status.Text=(F 'partial' @($script:items.Count))}
 finally{$ui.Save.IsEnabled=$script:items.Count -gt 0;$ui.Choose.IsEnabled=$true;$ui.Process.IsEnabled=$true;$ui.Photos.IsEnabled=$true;$ui.Progress.Visibility='Collapsed';$script:job.Dispose();$script:job=$null}
})
$ui.Photos.Add_SelectionChanged({try{Selected}catch{Fail $_.Exception.Message}})
function Rotate($delta){if($ui.Photos.SelectedIndex -lt 0){return};$item=$script:items[$ui.Photos.SelectedIndex];$item.Rotation=($item.Rotation+$delta+360)%360;$item.Thumb.Source=Bitmap $item.Path $item.Rotation 320;$script:dirty=$true;Selected;$ui.Status.Text=(T 'rotated')}
$ui.Left.Add_Click({try{Rotate -90}catch{Fail $_.Exception.Message}});$ui.Right.Add_Click({try{Rotate 90}catch{Fail $_.Exception.Message}})
$ui.Advanced.Add_Click({Start-Process -FilePath (Join-Path $script:runDir 'kontrola.html');[Windows.MessageBox]::Show($window,(T 'advancedInfo'),(T 'advancedTitle'),'OK','Information')|Out-Null})
function Export-Photos($dest){
 $codec=[Drawing.Imaging.ImageCodecInfo]::GetImageEncoders()|Where-Object MimeType -eq 'image/jpeg';$ep=[Drawing.Imaging.EncoderParameters]::new(1);$ep.Param[0]=[Drawing.Imaging.EncoderParameter]::new([Drawing.Imaging.Encoder]::Quality,[long]98)
 try{foreach($item in $script:items){$target=Join-Path $dest $item.Name;if($item.Rotation -eq 0){[IO.File]::Copy($item.Path,$target,$false);continue};$im=[Drawing.Bitmap]::new($item.Path);try{$im.RotateFlip([Drawing.RotateFlipType]([int]($item.Rotation/90)));$im.Save($target,$codec,$ep)}finally{$im.Dispose()}}}finally{$ep.Dispose()}
}
$ui.Save.Add_Click({try{
 $d=New-Object Windows.Forms.FolderBrowserDialog;$d.Description=(T 'folder');if($d.ShowDialog() -ne 'OK'){return}
 $dest=Join-Path $d.SelectedPath ((T 'folderPrefix')+(Get-Date -Format 'yyyy-MM-dd_HH-mm-ss')+'-'+[guid]::NewGuid().ToString('N').Substring(0,4));[IO.Directory]::CreateDirectory($dest)|Out-Null
 Export-Photos $dest
 $script:dirty=$false;$ui.Status.Text=(F 'saved' @($script:items.Count,$dest));[Windows.MessageBox]::Show($window,(F 'savedDialog' @($dest)),(T 'done'),'OK','Information')|Out-Null;Start-Process explorer.exe -ArgumentList ('"'+$dest+'"')
}catch{Fail ((T 'saveError')+$_.Exception.Message)}})
$window.Add_Closing({param($sender,$e)
 if($script:job -and !$script:job.HasExited){[Windows.MessageBox]::Show($window,(T 'wait'),(T 'app'))|Out-Null;$e.Cancel=$true;return}
 if($script:dirty -and [Windows.MessageBox]::Show($window,(T 'close'),(T 'unsaved'),'YesNo','Question') -ne 'Yes'){$e.Cancel=$true}
})
$window.ShowDialog()|Out-Null


