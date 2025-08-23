"""AFDB command line interface."""
import argparse, orjson, json
from .parse_mcu import parse_mcu
from .parse_ip import parse_ip
from .build_catalog import build_catalog, save_catalog
from .report import catalog_to_markdown
from .compact_ir import encode_and_compress


def dumps(obj) -> str:
    return orjson.dumps(obj, option=orjson.OPT_INDENT_2).decode()


def main():
    ap = argparse.ArgumentParser("afdb")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_mcu = sub.add_parser("import-mcu", help="Parse MCU XML into canonical JSON")
    p_mcu.add_argument("--in", dest="inp", required=True)
    p_mcu.add_argument("--out", dest="out", required=True)

    p_ip = sub.add_parser("import-ip", help="Parse IP Modes XML into canonical JSON")
    p_ip.add_argument("--in", dest="inp", required=True)
    p_ip.add_argument("--out", dest="out", required=True)

    p_cat = sub.add_parser("build-catalog", help="Fuse MCU + IP overlays")
    p_cat.add_argument("--mcu", required=True)
    p_cat.add_argument("--ip", required=False)
    p_cat.add_argument("--out", required=False)
    p_cat.add_argument("--afdb-root", required=False, help="Root directory for afdb/<family>/<part>.json")

    p_rep = sub.add_parser("report", help="Generate pin/function report from catalog")
    p_rep.add_argument("--catalog", required=True)
    p_rep.add_argument("--out", required=True)

    p_ir = sub.add_parser("encode-ir", help="Encode catalog to compact IR and compress")
    p_ir.add_argument("--catalog", required=True)
    p_ir.add_argument("--out", required=True)

    args = ap.parse_args()
    if args.cmd == "import-mcu":
        data = parse_mcu(args.inp)
        open(args.out, "wb").write(orjson.dumps(data, option=orjson.OPT_INDENT_2))
    elif args.cmd == "import-ip":
        data = parse_ip(args.inp)
        open(args.out, "wb").write(orjson.dumps(data, option=orjson.OPT_INDENT_2))
    elif args.cmd == "build-catalog":
        mcu = json.loads(open(args.mcu, "rb").read())
        ip = json.loads(open(args.ip, "rb").read()) if args.ip else None
        cat = build_catalog(mcu, ip)
        if args.afdb_root:
            save_catalog(cat, args.afdb_root)
        elif args.out:
            open(args.out, "wb").write(orjson.dumps(cat, option=orjson.OPT_INDENT_2))
        else:
            raise SystemExit("--out or --afdb-root required")
    elif args.cmd == "report":
        cat = json.loads(open(args.catalog, "rb").read())
        md = catalog_to_markdown(cat)
        open(args.out, "w").write(md)
    elif args.cmd == "encode-ir":
        cat = json.loads(open(args.catalog, "rb").read())
        blob = encode_and_compress(cat)
        open(args.out, "wb").write(blob)

if __name__ == "__main__":
    main()
