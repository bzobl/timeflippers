# Timeflippers - Using a TimeFlip2 without their app

The TimeFlip2[^1] uses BLE/GATT to read and write to the dice. The
protocol and attributes are listed here[^2].

# Connecting the TimeFlip2

We rely on bluez to communicate with the dice. In order to connect, the
following steps are necessary:

- open `bluetoothctl` interactively
- view the attached bluetooth controller: `devices`
- you can view more detailed information: `show 00:11:22:33:44:55`
- if more than one bluetooth controller is available, select one: `select 00:11:22:33:44:55`
- make sure you have power: `power on`
- start scanning for devices: `scan on`
- The TimeFlip dice should show up (you might have to move it to wake it
  up) as `[NEW] Device EB:12:A0:12:34:56 TimeFlip v2.0`
- pair with the dice: `pair EB:12:A0:12:34:56`
  (This should automatically connect it)
- turn off scanning: `scan off`
- check the dice with: `show EB:12:A0:12:34:56`, this should show some
  thing like
  ```
  Device EB:12:A0:12:34:56 (random)
    Name: TimeFlip v2.0
    Alias: TimeFlip v2.0
    Paired: yes
    Trusted: no
    Blocked: no
    Connected: yes
    LegacyPairing: no
    UUID: Generic Access Profile    (00001800-0000-1000-8000-00805f9b34fb)
    UUID: Generic Attribute Profile (00001801-0000-1000-8000-00805f9b34fb)
    UUID: Device Information        (0000180a-0000-1000-8000-00805f9b34fb)
    UUID: Battery Service           (0000180f-0000-1000-8000-00805f9b34fb)
    UUID: Nordic Semiconductor AS.. (0000fe59-0000-1000-8000-00805f9b34fb)
    UUID: Vendor specific           (f1196f50-71a4-11e6-bdf4-0800200c9a66)
    ManufacturerData Key: 0xffff
    ManufacturerData Value:
    54 2e 46 6c 69 70 00                             T.Flip.
    Battery Percentage: 0x64 (100)
  ```

# Read Characteristics

Once we're connected to the cube we can attempt to read characteristics.
We can, again, use `bluetoothctl` for that:

- enter the GATT menu: `menu gatt`
- to read, write or be notified about a characteristic, its UUID has to
  be selected: `select-attribtue <UUID>` and then a `read`, `write` or
  `notify` command has to be issued.

E.g., to read the battery status (UUID _00002a19-0000-1000-8000-00805f9b34fb_),
the following can be done:

```
[TimeFlip v2.0:/service0012/char0019]# select-attribute 00002a19-0000-1000-8000-00805f9b34fb
[TimeFlip v2.0:/service001d/char001e]# read
Attempting to read /org/bluez/hci0/dev_EB_12_A0_12_34_56/service001d/char001e
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service001d/char001e Value:
  64                                               d               
  64                                               d 
```

The single byte returned (0x64) is the current battery level, i.e., 100%.

The TimeFlip2 specific characteristics are accessible via UUIDs
_F1196F51-71A4-11E6-BDF4-0800200C9A66_ through _F1196F58-71A4-11E6-BDF4-0800200C9A66_
(note the last digit of the first UUID chunk), the semantics of those
can be found here[^2].

## First Steps

When reading the 'System state characteristic' the state of the dice can
be determined:
```
[TimeFlip v2.0:/service001d/char001e]# select-attribute F1196F56-71A4-11E6-BDF4-0800200C9A66
[TimeFlip v2.0:/service0021/char0030]# read
Attempting to read /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0030
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0030 Value:
  01 00 00 00                                      ....
  01 00 00 00                                      ....
```
This indicates that the device has been reset and has to be
reinizialized.

### Password

To access the dice, a password, which is reset on every disconnect, has
to be written to _F1196F57-71A4-11E6-BDF4-0800200C9A66_. The default
password is "000000", hence the initial authorization should be done as
follows:
```
[TimeFlip v2.0:/service0021/char0022]# select-attribute F1196F57-71A4-11E6-BDF4-0800200C9A66
[TimeFlip v2.0:/service0021/char0033]# write "0x30 0x30 0x30 0x30 0x30 0x30"
Attempting to write /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0033
[TimeFlip v2.0:/service0021/char0033]# select-attribute F1196F51-71A4-11E6-BDF4-0800200C9A66
[TimeFlip v2.0:/service0021/char0022]# read
Attempting to read /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0022
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0022 Value:
  70 61 73 73 77 6f 72 64 20 4f 4b                 password OK
  70 61 73 73 77 6f 72 64 20 4f 4b                 password OK
[TimeFlip v2.0:/service0021/char0022]# select-attribute F1196F53-71A4-11E6-BDF4-0800200C9A66
[TimeFlip v2.0:/service0021/char0028]# read
Attempting to read /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0028
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0028 Value:
  02                                               .
  02                                               .
```

Please note that the doc[^2] seems to be wrong, as the status 0x02 seems to be "password OK".

### Set time

Once the "app" has authorized, the dice's time can be set:
```
[TimeFlip v2.0:/service0021/char0028]# select-attribute F1196F54-71A4-11E6-BDF4-0800200C9A66
[TimeFlip v2.0:/service0021/char002b]# write "0x08 0x00 0x00 0x00 0x00 0x64 0x8A 0x35 0xA3"
Attempting to write /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char002b
```

### And so on..

This should probably be repeated for Facet colors, LED brightness, Blink
interval and so on.

## Get notified

To get notified about flips, select the 'Facets characteristic' and
turn notifications on
```
[TimeFlip v2.0:/service0021/char0030]# select-attribute F1196F52-71A4-11E6-BDF4-0800200C9A66
[TimeFlip v2.0:/service0021/char0025]# notify on
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0025 Notifying: yes
Notify started
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0025 Value:
  03                                               .
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0025 Value:
  02                                               .
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0025 Value:
  07                                               .
[CHG] Attribute /org/bluez/hci0/dev_EB_12_A0_12_34_56/service0021/char0025 Value:
  02                                               .
```
---

[^1][https://timeflip.io/]

[^2][https://github.com/DI-GROUP/TimeFlip.Docs/blob/master/Hardware/TimeFlip%20BLE%20protocol%20ver4_02.06.2020.md]
