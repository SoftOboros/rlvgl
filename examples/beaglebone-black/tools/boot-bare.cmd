echo "rlvgl-bbb-bare: lighting all 4 USR LEDs as a u-boot signature"
led usr0 on
led usr1 on
led usr2 on
led usr3 on
echo "rlvgl-bbb-bare: loading flat binary from SD FAT..."
fatload mmc 0:1 0x82000000 rlvgl-bbb-bare.bin
echo "rlvgl-bbb-bare: jumping to 0x82000000"
go 0x82000000
