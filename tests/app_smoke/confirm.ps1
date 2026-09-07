param([long]$WindowHandle)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$koreanOK = [string][char]0xD655 + [char]0xC778
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$WindowHandle)
$all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
$items = @()
$buttons = @()
$hasConfirmation = $false
foreach ($element in $all) {
    $name = $element.Current.Name
    $type = $element.Current.ControlType.ProgrammaticName
    if ($name -match 'Headphones compensation will be recorded') { $hasConfirmation = $true }
    if ($type -eq 'ControlType.Button' -and ($name -eq 'OK' -or $name -eq $koreanOK)) { $buttons += $element }
    $items += @{ name = $name; type = $type }
}
$accepted = $false
if ($hasConfirmation -and $buttons.Count -eq 1) {
    $pattern = $buttons[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $pattern.Invoke()
    $accepted = $true
}
@{ accepted = $accepted; confirmation = $hasConfirmation; elements = $items } | ConvertTo-Json -Depth 4 -Compress
