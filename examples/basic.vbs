Option Explicit
Randomize

ExecuteGlobal GetTextFile("controller.vbs")

Dim bFlippersEnabled

Sub Table1_Init
    debug.print "Hello, World!"
    'add a ball
    bFlippersEnabled = True
End Sub

Sub Table1_KeyDown(ByVal Keycode)
    debug.print "Down Keycode: " & Keycode
    If keycode = LeftFlipperKey and bFlippersEnabled Then
        LeftFlipper.RotateToEnd
    End If
    If keycode = RightFlipperKey and bFlippersEnabled Then
        RightFlipper.RotateToEnd
    End If
End Sub

Sub Table1_KeyUp(ByVal Keycode)
    debug.print "Up Keycode: " & Keycode
    If keycode = LeftFlipperKey and bFlippersEnabled Then
        LeftFlipper.RotateToStart
    End If
    If keycode = RightFlipperKey and bFlippersEnabled Then
        RightFlipper.RotateToStart
    End If
End Sub
