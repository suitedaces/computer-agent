import os
from PIL import Image

def screenshot(which='primary'):
    os.system("""
        powershell.exe \"
        Add-Type -AssemblyName System.Windows.Forms,System.Drawing

    \\$screens = [Windows.Forms.Screen]::AllScreens

    # Iterate through each screen
    foreach (\\$screen in \\$screens) {
        Write-Host "Monitor Name: " \\$screen.DeviceName
        Write-Host "Bounds: " \\$screen.Bounds
        Write-Host "Working Area: " \\$screen.WorkingArea
        Write-Host "Primary: " \\$(\\$screen.Primary)
        Write-Host "Bounds Top: " \\$screen.Bounds.Top
        Write-Host "Bounds Left: " \\$screen.Bounds.Left
        Write-Host "Bounds Right: " \\$screen.Bounds.Right
        Write-Host "Bounds Bottom: " \\$screen.Bounds.Bottom
        Write-Host "-----------------------------"

        \\$screenshot_dir = \\$env:USERPROFILE + \\\"\\Pictures\\Screenshots\\\"
        if (\\$screen.Primary) {
            Write-Host "Primary Monitor"
            \\$filename = \\$screenshot_dir + \\\"\\screenshot_primary.png\\\"
            
        } else {
            Write-Host "Secondary Monitor"
            \\$filename = \\$screenshot_dir + \\\"\\screenshot_secondary.png\\\"
        }
        \\$top    = (\\$screen.Bounds.Top    | Measure-Object -Minimum).Minimum
        \\$left   = (\\$screen.Bounds.Left   | Measure-Object -Minimum).Minimum
        \\$right  = (\\$screen.Bounds.Right  | Measure-Object -Maximum).Maximum
        \\$bottom = (\\$screen.Bounds.Bottom | Measure-Object -Maximum).Maximum

        \\$bounds   = [Drawing.Rectangle]::FromLTRB(\\$left, \\$top, \\$right, \\$bottom)
        \\$bmp      = New-Object System.Drawing.Bitmap ([int]\\$bounds.width), ([int]\\$bounds.height)
        \\$graphics = [Drawing.Graphics]::FromImage(\\$bmp)

        \\$graphics.CopyFromScreen(\\$bounds.Location, [Drawing.Point]::Empty, \\$bounds.size)

        Write-Host \\$filename
        \\$bmp.Save(\\$filename, [Drawing.Imaging.ImageFormat]::Png)
        

        \\$graphics.Dispose()
        \\$bmp.Dispose()
    }
    \"
    """)
    username = "Alhazen"
    file_path = "/mnt/c/Users/" + username + "/Pictures/Screenshots/"
    if which == 'primary':
        filename = file_path + "screenshot_primary.png"
    else:
        filename = file_path + "screenshot_secondary.png"
    im = Image.open(filename)
    return im

if __name__ == "__main__":
    screenshot()
