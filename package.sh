#!/bin/sh -e

ABL2ESP="./target/aarch64-unknown-uefi/debug/abl2esp.efi"

rm -f packaging/f536d559-459f-48fa-8bbc-43b554ecae8dSEC2.1.pe32
rm -f packaging/f536d559-459f-48fa-8bbc-43b554ecae8dSEC1.ui
rm -f packaging/f536d559-459f-48fa-8bbc-43b554ecae8d.ffs
rm -f packaging/FVMAIN.Fv
rm -f packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.1fv.sec
rm -f packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.guided.dummy
rm -f packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.tmp
rm -f packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.guided
rm -f packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792.ffs
rm -f packaging/FVMAIN_COMPACT.Fv
rm -f abl-unsigned.elf

GenSec -s EFI_SECTION_PE32 -o packaging/f536d559-459f-48fa-8bbc-43b554ecae8dSEC2.1.pe32 ${ABL2ESP}
GenSec -o packaging/f536d559-459f-48fa-8bbc-43b554ecae8dSEC1.ui -s EFI_SECTION_USER_INTERFACE -n LinuxLoader
GenFfs -t EFI_FV_FILETYPE_APPLICATION -g f536d559-459f-48fa-8bbc-43b554ecae8d -o packaging/f536d559-459f-48fa-8bbc-43b554ecae8d.ffs -i packaging/f536d559-459f-48fa-8bbc-43b554ecae8dSEC1.ui -i packaging/f536d559-459f-48fa-8bbc-43b554ecae8dSEC2.1.pe32
GenFv -a packaging/Ffs-FVMAIN.inf -o packaging/FVMAIN.Fv -i packaging/FVMAIN.inf
GenSec -s EFI_SECTION_FIRMWARE_VOLUME_IMAGE -o packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.1fv.sec packaging/FVMAIN.Fv
GenSec --sectionalign 8 -o packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.guided.dummy packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.1fv.sec
LzmaCompress -e -o packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.tmp packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.guided.dummy
GenSec -s EFI_SECTION_GUID_DEFINED -g EE4E5898-3914-4259-9D6E-DC7BD79403CF -r PROCESSING_REQUIRED -o packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.guided packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.tmp
GenFfs -t EFI_FV_FILETYPE_FIRMWARE_VOLUME_IMAGE -g 9E21FD93-9C72-4c15-8C4B-E77F1DB2D792 -o packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792.ffs -i packaging/9E21FD93-9C72-4c15-8C4B-E77F1DB2D792SEC1.guided
GenFv -a packaging/FVMAIN_COMPACT.inf -o packaging/FVMAIN_COMPACT.Fv -i packaging/FVMAIN_COMPACT.inf
packaging/image_header.py packaging/FVMAIN_COMPACT.Fv abl-unsigned.elf 0X9fa00000 elf 32 nohash
