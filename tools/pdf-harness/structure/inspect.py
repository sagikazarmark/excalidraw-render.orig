#!/usr/bin/env python3

import base64
import hashlib
import json
import os
import struct
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlsplit


OPERATORS = {
    "b",
    "b*",
    "B",
    "B*",
    "BDC",
    "BI",
    "BMC",
    "BT",
    "BX",
    "c",
    "cm",
    "CS",
    "cs",
    "d",
    "d0",
    "d1",
    "Do",
    "DP",
    "EI",
    "EMC",
    "ET",
    "EX",
    "f",
    "F",
    "f*",
    "G",
    "g",
    "gs",
    "h",
    "i",
    "ID",
    "J",
    "j",
    "K",
    "k",
    "l",
    "m",
    "M",
    "MP",
    "n",
    "q",
    "Q",
    "re",
    "RG",
    "rg",
    "ri",
    "s",
    "S",
    "SC",
    "sc",
    "SCN",
    "scn",
    "sh",
    "T*",
    "Tc",
    "Td",
    "TD",
    "Tf",
    "Tj",
    "TJ",
    "TL",
    "Tm",
    "Tr",
    "Ts",
    "Tw",
    "Tz",
    "v",
    "w",
    "W",
    "W*",
    "y",
    "'",
    '"',
}

VECTOR_OPERATORS = {
    "m",
    "l",
    "c",
    "v",
    "y",
    "h",
    "re",
    "S",
    "s",
    "f",
    "F",
    "f*",
    "B",
    "B*",
    "b",
    "b*",
    "n",
    "W",
    "W*",
}

TEXT_OPERATORS = {
    "BT",
    "ET",
    "Tc",
    "Tw",
    "Tz",
    "TL",
    "Tf",
    "Tr",
    "Ts",
    "Td",
    "TD",
    "Tm",
    "T*",
    "Tj",
    "TJ",
    "'",
    '"',
}


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: inspect.py <input.pdf> <expectation.json> <pass|fail> <output.json>",
            file=sys.stderr,
        )
        return 2
    input_path = Path(sys.argv[1]).resolve(strict=True)
    expectation_path = Path(sys.argv[2]).resolve(strict=True)
    expected_pass = {"pass": True, "fail": False}.get(sys.argv[3])
    if expected_pass is None:
        raise RuntimeError("expected outcome must be pass or fail")
    output_path = Path(sys.argv[4]).resolve()
    qpdf = Path(os.environ["QPDF"]).resolve(strict=True)
    expectation = json.loads(expectation_path.read_text(encoding="utf-8"))
    report = inspect(qpdf, input_path, expectation, expected_pass)
    output_path.write_text(
        json.dumps(report, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        + "\n",
        encoding="utf-8",
    )
    return 0 if report["passed"] == expected_pass else 1


def inspect(qpdf: Path, input_path: Path, expectation: dict, expected_pass: bool) -> dict:
    raw_pdf = input_path.read_bytes()
    check = subprocess.run(
        [str(qpdf), "--check", str(input_path)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    warnings = normalized_messages(check.stderr, input_path)
    parser_status = {0: "clean", 2: "error", 3: "warning"}.get(
        check.returncode, "error"
    )
    errors = [] if parser_status != "error" else warnings

    json_run = subprocess.run(
        [
            str(qpdf),
            "--json=2",
            "--json-stream-data=inline",
            "--decode-level=generalized",
            str(input_path),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    json_messages = normalized_messages(json_run.stderr, input_path)
    for message in json_messages:
        if message not in warnings:
            warnings.append(message)
    if json_run.returncode not in (0, 3):
        errors.extend(message for message in json_messages if message not in errors)
    try:
        source = json.loads(json_run.stdout)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        errors.append(f"qpdf JSON parse failed: {error}")
        source = empty_source()

    report = build_report(source, raw_pdf, parser_status, warnings, errors)
    classify_images(report, expectation)
    report["checks"] = evaluate(report, expectation)
    report["expected_pass"] = expected_pass
    report["passed"] = parser_status == "clean" and not errors and all(
        check["passed"] for check in report["checks"]
    )
    return report


def normalized_messages(data: bytes, input_path: Path) -> list[str]:
    text = data.decode("utf-8", errors="replace").replace(str(input_path), "<input.pdf>")
    return [line for line in text.splitlines() if line]


def empty_source() -> dict:
    return {
        "acroform": {"fields": [], "hasacroform": False},
        "attachments": {},
        "encrypt": {"encrypted": False},
        "pages": [],
        "qpdf": [{"pdfversion": None}, {"trailer": {"value": {}}}],
    }


def build_report(
    source: dict,
    raw_pdf: bytes,
    parser_status: str,
    warnings: list[str],
    errors: list[str],
) -> dict:
    errors = list(errors)
    qpdf_records = source.get("qpdf", [{}, {}])
    identity = qpdf_records[0] if qpdf_records else {}
    objects = qpdf_records[1] if len(qpdf_records) > 1 else {}
    trailer = object_value(objects.get("trailer", {}))
    catalog = resolve(objects, trailer.get("/Root"))
    pages = []
    for page_record in source.get("pages", []):
        page_ref = page_record.get("object")
        page = resolve(objects, page_ref)
        resources = resolve_value(
            objects, inherited_page_value(objects, page, "/Resources", {})
        )
        media_box = inherited_page_value(objects, page, "/MediaBox", None)
        crop_box = inherited_page_value(objects, page, "/CropBox", media_box)
        other_boxes = {}
        for name in ("/BleedBox", "/TrimBox", "/ArtBox"):
            value = inherited_page_value(objects, page, name, None)
            if value is not None:
                other_boxes[name[1:]] = value
        operations = content_operations(
            objects, page_record.get("contents", []), resources, errors
        )
        fonts, mappings = font_inventory(objects, resources, operations)
        images = image_inventory(objects, resources, operations)
        transparency = transparency_inventory(objects, resources)
        forms = form_inventory(objects, resources, operations)
        clips = [operation for operation in operations if operation["operator"] in ("W", "W*")]
        masks = [record for record in transparency if record["soft_mask"] is not None]
        masks.extend(
            {
                "image_object": image["object"],
                "kind": "image-soft-mask",
                "resource": image["resource"],
                "soft_mask": image["soft_mask"],
            }
            for image in images
            if image["soft_mask"] is not None
        )
        pages.append(
            {
                "annotations": annotation_inventory(objects, page),
                "clips": clips,
                "crop_box": crop_box,
                "fonts": fonts,
                "forms": forms,
                "images": images,
                "mappings": mappings,
                "media_box": media_box,
                "masks": masks,
                "operations": operations,
                "other_boxes": other_boxes,
                "rotate": inherited_page_value(objects, page, "/Rotate", 0),
                "text_operations": [
                    operation
                    for operation in operations
                    if operation["operator"] in TEXT_OPERATORS
                ],
                "transparency": transparency,
                "user_unit": page.get("/UserUnit", 1),
                "vector_operations": [
                    operation
                    for operation in operations
                    if operation["operator"] in VECTOR_OPERATORS
                ],
            }
        )

    output_intents = output_intent_inventory(objects, catalog)
    actions = action_inventory(objects, catalog, source.get("pages", []))
    metadata_stream = stream_value(objects, catalog.get("/Metadata"))
    attachments = attachment_inventory(source, objects)
    return {
        "checks": [],
        "document": {
            "attachments": attachments,
            "actions": actions,
            "encrypted": bool(source.get("encrypt", {}).get("encrypted", False)),
            "file_ids": normalize(trailer.get("/ID")),
            "incremental_revisions": max(raw_pdf.count(b"startxref") - 1, 0),
            "info_dictionary": normalize(resolve(objects, trailer.get("/Info")))
            if trailer.get("/Info")
            else None,
            "linearized": b"/Linearized" in raw_pdf[:1024],
            "output_intents": output_intents,
            "page_count": len(pages),
            "pdf_version": identity.get("pdfversion"),
            "signatures": signature_inventory(source, objects),
            "xmp": stream_descriptor(metadata_stream),
        },
        "errors": errors,
        "expected_pass": False,
        "pages": pages,
        "parser": {
            "name": "qpdf",
            "status": parser_status,
            "version": "12.3.2",
            "warnings": warnings,
        },
        "passed": False,
        "schema": "excalidraw-render/pdf-structure-report/v1",
    }


def content_operations(
    objects: dict, content_refs: list, resources: dict, errors: list[str]
) -> list[dict]:
    result = []
    sequence = [0]
    walk_contents(
        objects,
        content_refs,
        resources,
        "page",
        [],
        result,
        sequence,
        errors,
    )
    return result


def walk_contents(
    objects: dict,
    content_refs: list,
    resources: dict,
    scope: str,
    form_stack: list[str],
    result: list[dict],
    sequence: list[int],
    errors: list[str],
) -> None:
    xobjects = resolve_value(objects, resources.get("/XObject", {}))
    states = resolve_value(objects, resources.get("/ExtGState", {}))
    for content_ref in content_refs:
        stream = stream_value(objects, content_ref)
        if stream is None:
            continue
        operands = []
        for token_kind, token in tokenize(stream):
            if token_kind == "word" and token in OPERATORS:
                operation = {
                    "operands": operands,
                    "operator": token,
                    "scope": scope,
                    "sequence": sequence[0],
                }
                result.append(operation)
                sequence[0] += 1
                if token == "Do" and operands:
                    resource = operands[-1]
                    form_ref = xobjects.get(resource) if isinstance(xobjects, dict) else None
                    form = stream_dict(objects, form_ref)
                    if form.get("/Subtype") == "/Form":
                        if form_ref in form_stack:
                            errors.append(f"recursive Form XObject {form_ref} in {scope}")
                        elif len(form_stack) >= 32:
                            errors.append(f"Form XObject depth exceeds 32 in {scope}")
                        else:
                            form_resources = resolve_value(
                                objects, form.get("/Resources", resources)
                            )
                            walk_contents(
                                objects,
                                [form_ref],
                                form_resources,
                                f"{scope}{resource}",
                                [*form_stack, form_ref],
                                result,
                                sequence,
                                errors,
                            )
                elif token == "gs" and operands:
                    resource = operands[-1]
                    state_ref = states.get(resource) if isinstance(states, dict) else None
                    state = resolve(objects, state_ref)
                    soft_mask = resolve_value(objects, state.get("/SMask"))
                    mask_form_ref = soft_mask.get("/G") if isinstance(soft_mask, dict) else None
                    mask_form = stream_dict(objects, mask_form_ref)
                    if mask_form.get("/Subtype") == "/Form":
                        if mask_form_ref in form_stack:
                            errors.append(f"recursive soft-mask Form {mask_form_ref} in {scope}")
                        elif len(form_stack) >= 32:
                            errors.append(f"soft-mask Form depth exceeds 32 in {scope}")
                        else:
                            mask_resources = resolve_value(
                                objects, mask_form.get("/Resources", {})
                            )
                            walk_contents(
                                objects,
                                [mask_form_ref],
                                mask_resources,
                                f"{scope}{resource}/SMask",
                                [*form_stack, mask_form_ref],
                                result,
                                sequence,
                                errors,
                            )
                operands = []
            else:
                operands.append(token)


def tokenize(data: bytes):
    delimiters = b"()<>[]{}/%"
    whitespace = b"\0\t\n\f\r "
    index = 0
    while index < len(data):
        byte = data[index]
        if byte in whitespace:
            index += 1
            continue
        if byte == ord("%"):
            newline = data.find(b"\n", index)
            index = len(data) if newline < 0 else newline + 1
            continue
        if byte == ord("("):
            start = index
            depth = 1
            index += 1
            while index < len(data) and depth:
                if data[index] == ord("\\"):
                    index += 2
                    continue
                if data[index] == ord("("):
                    depth += 1
                elif data[index] == ord(")"):
                    depth -= 1
                index += 1
            yield "string", data[start:index].decode("latin-1")
            continue
        if byte == ord("<") and index + 1 < len(data) and data[index + 1] == ord("<"):
            yield "delimiter", "<<"
            index += 2
            continue
        if byte == ord(">") and index + 1 < len(data) and data[index + 1] == ord(">"):
            yield "delimiter", ">>"
            index += 2
            continue
        if byte == ord("<"):
            end = data.find(b">", index + 1)
            end = len(data) - 1 if end < 0 else end
            yield "string", data[index : end + 1].decode("latin-1")
            index = end + 1
            continue
        if byte == ord("/"):
            end = index + 1
            while end < len(data) and data[end] not in whitespace + delimiters:
                end += 1
            yield "name", data[index:end].decode("latin-1")
            index = end
            continue
        if byte in b"[]{}":
            yield "delimiter", chr(byte)
            index += 1
            continue
        if byte in b")>":
            yield "delimiter", chr(byte)
            index += 1
            continue
        end = index
        while end < len(data) and data[end] not in whitespace + delimiters:
            end += 1
        token = data[index:end].decode("latin-1")
        yield "word", token
        index = end
        if token == "ID":
            if index < len(data) and data[index] in whitespace:
                index += 1
            data_start = index
            data_end = None
            while index + 1 < len(data):
                if (
                    data[index : index + 2] == b"EI"
                    and (index == data_start or data[index - 1] in whitespace)
                    and (index + 2 == len(data) or data[index + 2] in whitespace)
                ):
                    data_end = index
                    if data_end > data_start and data[data_end - 1] in whitespace:
                        data_end -= 1
                    break
                index += 1
            if data_end is None:
                inline = data[data_start:]
                index = len(data)
            else:
                inline = data[data_start:data_end]
                index += 2
            yield "inline-data", f"inline:{len(inline)}:{sha256(inline)}"
            if data_end is not None:
                yield "word", "EI"


def font_inventory(
    objects: dict, resources: dict, operations: list[dict]
) -> tuple[list[dict], list[dict]]:
    result = []
    mappings = []
    for scope, scoped_resources in resource_scopes(objects, resources):
        fonts = resolve_value(objects, scoped_resources.get("/Font", {}))
        for resource, font_ref in sorted_dict(fonts):
            full_resource = f"{scope}{resource}"
            font = resolve(objects, font_ref)
            descendant = {}
            descendants = font.get("/DescendantFonts", [])
            if descendants:
                descendant = resolve(objects, descendants[0])
            descriptor = resolve(
                objects, descendant.get("/FontDescriptor", font.get("/FontDescriptor"))
            )
            embedded_ref = next(
                (
                    descriptor[name]
                    for name in ("/FontFile", "/FontFile2", "/FontFile3")
                    if name in descriptor
                ),
                None,
            )
            embedded = stream_value(objects, embedded_ref)
            to_unicode = stream_value(objects, font.get("/ToUnicode"))
            result.append(
                {
                    "base_font": normalize(font.get("/BaseFont")),
                    "descendant_subtype": normalize(descendant.get("/Subtype")),
                    "embedded": embedded is not None,
                    "embedded_sha256": sha256(embedded) if embedded is not None else None,
                    "encoding": normalize(font.get("/Encoding")),
                    "object": reference(font_ref),
                    "resource": full_resource,
                    "subtype": normalize(font.get("/Subtype")),
                    "to_unicode": to_unicode is not None,
                    "to_unicode_byte_length": len(to_unicode) if to_unicode is not None else None,
                    "to_unicode_sha256": sha256(to_unicode) if to_unicode is not None else None,
                }
            )
            mappings.append(
                {
                    "byte_length": len(to_unicode) if to_unicode is not None else None,
                    "kind": "to-unicode",
                    "operands": [],
                    "resource": full_resource,
                    "scope": scope,
                    "sequence": None,
                    "sha256": sha256(to_unicode) if to_unicode is not None else None,
                }
            )
    for operation in operations:
        if operation["operator"] == "BDC" and "/ActualText" in operation["operands"]:
            mappings.append(
                {
                    "byte_length": None,
                    "kind": "actual-text",
                    "operands": operation["operands"],
                    "resource": None,
                    "scope": operation["scope"],
                    "sequence": operation["sequence"],
                    "sha256": None,
                }
            )
    return result, mappings


def image_inventory(objects: dict, resources: dict, operations: list[dict]) -> list[dict]:
    result = []
    for scope, scoped_resources in resource_scopes(objects, resources):
        xobjects = resolve_value(objects, scoped_resources.get("/XObject", {}))
        for resource, image_ref in sorted_dict(xobjects):
            image = stream_dict(objects, image_ref)
            if image.get("/Subtype") != "/Image":
                continue
            full_resource = f"{scope}{resource}"
            append_image(
                objects,
                image_ref,
                full_resource,
                sum(
                    operation["operator"] == "Do"
                    and operation["scope"] == scope
                    and operation["operands"]
                    and operation["operands"][-1] == resource
                    for operation in operations
                ),
                result,
                {image_ref},
            )
    for index, operation in enumerate(operations):
        if operation["operator"] != "ID":
            continue
        inline_dict = inline_dictionary(operation["operands"])
        inline_data = None
        if index + 1 < len(operations) and operations[index + 1]["operator"] == "EI":
            inline_data = next(
                (
                    operand
                    for operand in operations[index + 1]["operands"]
                    if operand.startswith("inline:")
                ),
                None,
            )
        data_length = None
        data_sha256 = None
        if inline_data is not None:
            _, length, data_sha256 = inline_data.split(":", 2)
            data_length = int(length)
        scope = operation["scope"]
        result.append(
            {
                "bits_per_component": integer_value(
                    inline_dict.get("/BPC", inline_dict.get("/BitsPerComponent"))
                ),
                "classification": "undeclared",
                "color_space": inline_dict.get("/CS", inline_dict.get("/ColorSpace")),
                "data_sha256": data_sha256,
                "filters": inline_dict.get("/F", inline_dict.get("/Filter")),
                "height": integer_value(inline_dict.get("/H", inline_dict.get("/Height"))),
                "interpolate": inline_dict.get("/I", inline_dict.get("/Interpolate"))
                == "true",
                "mask": inline_dict.get("/IM", inline_dict.get("/ImageMask")),
                "object": None,
                "occurrences": 1,
                "resource": f"{scope}/inline-image-{operation['sequence']}",
                "soft_mask": None,
                "width": integer_value(inline_dict.get("/W", inline_dict.get("/Width"))),
                "byte_length": data_length,
            }
        )
    return result


def append_image(
    objects: dict,
    image_ref,
    resource: str,
    occurrences: int,
    result: list[dict],
    mask_stack: set[str],
) -> None:
    image = stream_dict(objects, image_ref)
    image_data = stream_value(objects, image_ref)
    result.append(
        {
            "bits_per_component": image.get("/BitsPerComponent"),
            "byte_length": len(image_data) if image_data is not None else None,
            "classification": "undeclared",
            "color_space": normalize(image.get("/ColorSpace")),
            "data_sha256": sha256(image_data) if image_data is not None else None,
            "filters": normalize(image.get("/Filter")),
            "height": image.get("/Height"),
            "interpolate": image.get("/Interpolate", False),
            "mask": normalize(image.get("/Mask")),
            "object": reference(image_ref),
            "occurrences": occurrences,
            "resource": resource,
            "soft_mask": reference(image.get("/SMask")),
            "width": image.get("/Width"),
        }
    )
    for name, mask_ref in (("Mask", image.get("/Mask")), ("SMask", image.get("/SMask"))):
        if not isinstance(mask_ref, str) or mask_ref in mask_stack:
            continue
        mask = stream_dict(objects, mask_ref)
        if mask.get("/Subtype") != "/Image":
            continue
        append_image(
            objects,
            mask_ref,
            f"{resource}/{name}",
            occurrences,
            result,
            {*mask_stack, mask_ref},
        )


def inline_dictionary(operands: list[str]) -> dict:
    result = {}
    index = 0
    while index + 1 < len(operands):
        if operands[index].startswith("/"):
            result[operands[index]] = operands[index + 1]
            index += 2
        else:
            index += 1
    return result


def integer_value(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def transparency_inventory(objects: dict, resources: dict) -> list[dict]:
    result = []
    for scope, scoped_resources in resource_scopes(objects, resources):
        states = resolve_value(objects, scoped_resources.get("/ExtGState", {}))
        for resource, state_ref in sorted_dict(states):
            state = resolve(objects, state_ref)
            soft_mask = state.get("/SMask")
            result.append(
                {
                    "alpha_is_shape": state.get("/AIS", False),
                    "blend_mode": normalize(state.get("/BM", "/Normal")),
                    "fill_alpha": state.get("/ca", 1),
                    "object": reference(state_ref),
                    "resource": f"{scope}{resource}",
                    "soft_mask": None
                    if soft_mask in (None, "/None")
                    else normalize(resolve_value(objects, soft_mask)),
                    "stroke_alpha": state.get("/CA", 1),
                }
            )
    return result


def form_inventory(objects: dict, resources: dict, operations: list[dict]) -> list[dict]:
    result = []
    for scope, scoped_resources in resource_scopes(objects, resources):
        xobjects = resolve_value(objects, scoped_resources.get("/XObject", {}))
        for resource, form_ref in sorted_dict(xobjects):
            form = stream_dict(objects, form_ref)
            if form.get("/Subtype") != "/Form":
                continue
            group = resolve_value(objects, form.get("/Group"))
            result.append(
                form_record(
                    form,
                    form_ref,
                    f"{scope}{resource}",
                    sum(
                        operation["operator"] == "Do"
                        and operation["scope"] == scope
                        and operation["operands"]
                        and operation["operands"][-1] == resource
                        for operation in operations
                    ),
                    group,
                )
            )
        states = resolve_value(objects, scoped_resources.get("/ExtGState", {}))
        for resource, state_ref in sorted_dict(states):
            state = resolve(objects, state_ref)
            soft_mask = resolve_value(objects, state.get("/SMask"))
            form_ref = soft_mask.get("/G") if isinstance(soft_mask, dict) else None
            form = stream_dict(objects, form_ref)
            if form.get("/Subtype") != "/Form":
                continue
            group = resolve_value(objects, form.get("/Group"))
            result.append(
                form_record(
                    form,
                    form_ref,
                    f"{scope}{resource}/SMask",
                    sum(
                        operation["operator"] == "gs"
                        and operation["scope"] == scope
                        and operation["operands"]
                        and operation["operands"][-1] == resource
                        for operation in operations
                    ),
                    group,
                )
            )
    return result


def form_record(
    form: dict, form_ref, resource: str, occurrences: int, group
) -> dict:
    return {
        "bbox": normalize(form.get("/BBox")),
        "group": {
            "color_space": normalize(group.get("/CS")),
            "isolated": group.get("/I", False),
            "knockout": group.get("/K", False),
            "subtype": normalize(group.get("/S")),
        }
        if isinstance(group, dict)
        else None,
        "matrix": normalize(form.get("/Matrix", [1, 0, 0, 1, 0, 0])),
        "object": reference(form_ref),
        "occurrences": occurrences,
        "resource": resource,
    }


def resource_scopes(objects: dict, resources: dict) -> list[tuple[str, dict]]:
    result = []

    def visit(scope: str, current: dict, stack: set[str]) -> None:
        result.append((scope, current))
        xobjects = resolve_value(objects, current.get("/XObject", {}))
        for resource, form_ref in sorted_dict(xobjects):
            form = stream_dict(objects, form_ref)
            if form.get("/Subtype") != "/Form" or form_ref in stack:
                continue
            nested = resolve_value(objects, form.get("/Resources", {}))
            if isinstance(nested, dict):
                visit(f"{scope}{resource}", nested, {*stack, form_ref})
        states = resolve_value(objects, current.get("/ExtGState", {}))
        for resource, state_ref in sorted_dict(states):
            state = resolve(objects, state_ref)
            soft_mask = resolve_value(objects, state.get("/SMask"))
            form_ref = soft_mask.get("/G") if isinstance(soft_mask, dict) else None
            form = stream_dict(objects, form_ref)
            if form.get("/Subtype") != "/Form" or form_ref in stack:
                continue
            nested = resolve_value(objects, form.get("/Resources", {}))
            if isinstance(nested, dict):
                visit(f"{scope}{resource}/SMask", nested, {*stack, form_ref})

    visit("page", resources, set())
    return result


def annotation_inventory(objects: dict, page: dict) -> list[dict]:
    result = []
    for annotation_ref in page.get("/Annots", []):
        annotation = resolve(objects, annotation_ref)
        action = resolve_value(objects, annotation.get("/A"))
        result.append(
            {
                "action": {
                    "kind": normalize(action.get("/S")),
                    "uri": normalize(action.get("/URI")),
                }
                if action
                else None,
                "additional_actions": normalize(
                    resolve_value(objects, annotation.get("/AA"))
                ),
                "border": normalize(annotation.get("/Border")),
                "flags": annotation.get("/F", 0),
                "object": reference(annotation_ref),
                "quad_points": normalize(annotation.get("/QuadPoints")),
                "rect": normalize(annotation.get("/Rect")),
                "subtype": normalize(annotation.get("/Subtype")),
            }
        )
    return result


def action_inventory(objects: dict, catalog: dict, page_records: list[dict]) -> list[dict]:
    result = []
    walk_action(objects, catalog.get("/OpenAction"), "catalog/OpenAction", set(), result)
    walk_additional_actions(objects, catalog.get("/AA"), "catalog/AA", result)
    for page_index, page_record in enumerate(page_records):
        page = resolve(objects, page_record.get("object"))
        walk_additional_actions(objects, page.get("/AA"), f"page[{page_index}]/AA", result)
        for annotation_index, annotation_ref in enumerate(page.get("/Annots", [])):
            annotation = resolve(objects, annotation_ref)
            location = f"page[{page_index}]/annotation[{annotation_index}]"
            walk_action(objects, annotation.get("/A"), f"{location}/A", set(), result)
            walk_additional_actions(objects, annotation.get("/AA"), f"{location}/AA", result)
    return result


def walk_additional_actions(
    objects: dict, value, location: str, result: list[dict]
) -> None:
    actions = resolve_value(objects, value)
    if not isinstance(actions, dict):
        return
    for event, action in sorted_dict(actions):
        walk_action(objects, action, f"{location}{event}", set(), result)


def walk_action(
    objects: dict,
    value,
    location: str,
    seen: set[str],
    result: list[dict],
) -> None:
    if isinstance(value, list):
        for index, action in enumerate(value):
            walk_action(objects, action, f"{location}[{index}]", seen, result)
        return
    action_ref = reference(value)
    if action_ref is not None:
        if action_ref in seen:
            result.append(
                {
                    "classification": "prohibited",
                    "details": {},
                    "kind": "cycle",
                    "location": location,
                    "object": action_ref,
                }
            )
            return
        seen = {*seen, action_ref}
    action = resolve_value(objects, value)
    if not isinstance(action, dict) or "/S" not in action:
        return
    kind = normalize(action.get("/S"))
    uri = normalize(action.get("/URI"))
    uri_scheme = urlsplit(uri).scheme.lower() if isinstance(uri, str) else None
    safe_uri = kind == "/URI" and uri_scheme in ("", "http", "https", "mailto")
    result.append(
        {
            "classification": "uri" if safe_uri else "prohibited",
            "details": normalize(
                {
                    key[1:]: item
                    for key, item in action.items()
                    if key not in ("/S", "/Next")
                }
            ),
            "kind": kind,
            "location": location,
            "object": action_ref,
        }
    )
    walk_action(objects, action.get("/Next"), f"{location}/Next", seen, result)


def output_intent_inventory(objects: dict, catalog: dict) -> list[dict]:
    result = []
    for intent_ref in catalog.get("/OutputIntents", []):
        intent = resolve(objects, intent_ref)
        profile_ref = intent.get("/DestOutputProfile")
        profile = stream_value(objects, profile_ref)
        result.append(
            {
                "condition_identifier": normalize(
                    intent.get("/OutputConditionIdentifier")
                ),
                "info": normalize(intent.get("/Info")),
                "object": reference(intent_ref),
                "profile": icc_descriptor(profile),
                "profile_object": reference(profile_ref),
                "subtype": normalize(intent.get("/S")),
            }
        )
    return result


def attachment_inventory(source: dict, objects: dict) -> dict:
    result = {}
    for name, attachment in sorted_dict(source.get("attachments", {})):
        record = normalize(attachment)
        stream_ref = attachment.get("preferredcontents")
        data = stream_value(objects, stream_ref)
        record["byte_length"] = len(data) if data is not None else None
        record["sha256"] = sha256(data) if data is not None else None
        result[name] = record
    return result


def icc_descriptor(data: bytes | None) -> dict | None:
    if data is None:
        return None
    descriptor = {"byte_length": len(data), "sha256": sha256(data)}
    if len(data) < 132:
        descriptor["valid_header"] = False
        return descriptor
    count = struct.unpack(">I", data[128:132])[0]
    tags = []
    for index in range(count):
        start = 132 + index * 12
        if start + 12 > len(data):
            break
        tags.append(data[start : start + 4].decode("ascii", errors="replace"))
    descriptor.update(
        {
            "color_space": data[16:20].decode("ascii", errors="replace").rstrip(),
            "device_class": data[12:16].decode("ascii", errors="replace"),
            "pcs": data[20:24].decode("ascii", errors="replace").rstrip(),
            "signature": data[36:40].decode("ascii", errors="replace"),
            "tags": tags,
            "valid_header": struct.unpack(">I", data[:4])[0] == len(data),
            "version": f"{data[8]}.{data[9] >> 4}",
        }
    )
    return descriptor


def signature_inventory(source: dict, objects: dict) -> list[dict]:
    result = []
    for field in source.get("acroform", {}).get("fields", []):
        if field.get("fieldtype") == "/Sig":
            result.append(normalize(field))
    for object_name, record in sorted_dict(objects):
        value = object_value(record)
        if isinstance(value, dict) and "/ByteRange" in value:
            result.append(
                {"byte_range": normalize(value["/ByteRange"]), "object": object_name[4:]}
            )
    return result


def classify_images(report: dict, expectation: dict) -> None:
    declared = {
        image["resource"]: image["classification"]
        for image in expectation.get("images", [])
    }
    for page in report["pages"]:
        for image in page["images"]:
            image["classification"] = declared.get(image["resource"], "undeclared")


def evaluate(report: dict, expectation: dict) -> list[dict]:
    pages = report["pages"]
    first = pages[0] if pages else {}
    checks = []

    def check(name: str, actual, expected) -> None:
        checks.append(
            {"actual": actual, "expected": expected, "id": name, "passed": actual == expected}
        )

    check("parser-warnings", report["parser"]["warnings"], [])
    check("parser-errors", report["errors"], [])
    check("pdf-version", report["document"]["pdf_version"], expectation["pdf_version"])
    check("page-count", report["document"]["page_count"], expectation["page_count"])
    if pages:
        check("media-box", first["media_box"], expectation["media_box"])
        check("crop-box", first["crop_box"], expectation["crop_box"])
        check("other-page-boxes", first["other_boxes"], {})
        check("rotation", first["rotate"], 0)
        check("user-unit", first["user_unit"], 1)
        check("font-count", len(first["fonts"]), expectation["font_count"])
        check("form-count", len(first["forms"]), expectation["form_count"])
        check("mapping-count", len(first["mappings"]), expectation["mapping_count"])
        check(
            "annotation-count",
            len(first["annotations"]),
            expectation["annotation_count"],
        )
        check("clip-count", len(first["clips"]), expectation["clip_count"])
        check("mask-count", len(first["masks"]), expectation["mask_count"])
        check("image-count", len(first["images"]), expectation["image_count"])
        check(
            "image-inventory",
            [
                {
                    "classification": image["classification"],
                    "resource": image["resource"],
                }
                for image in first["images"]
            ],
            expectation["images"],
        )
        check(
            "operation-count",
            len(first["operations"]),
            expectation["operation_count"],
        )
        check(
            "operation-sequence-sha256",
            sha256(
                json.dumps(
                    first["operations"],
                    ensure_ascii=True,
                    separators=(",", ":"),
                    sort_keys=True,
                ).encode()
            ),
            expectation["operation_sha256"],
        )
        inventory = {
            "document": report["document"],
            "page": {
                key: value
                for key, value in first.items()
                if key not in ("operations", "text_operations", "vector_operations")
            },
        }
        check(
            "structural-inventory-sha256",
            sha256(
                json.dumps(
                    inventory,
                    ensure_ascii=True,
                    separators=(",", ":"),
                    sort_keys=True,
                ).encode()
            ),
            expectation["inventory_sha256"],
        )
        counts = {}
        for operation in first["operations"]:
            operator = operation["operator"]
            counts[operator] = counts.get(operator, 0) + 1
        for operator, minimum in expectation["operation_minimums"].items():
            actual = counts.get(operator, 0)
            checks.append(
                {
                    "actual": actual,
                    "expected": f">={minimum}",
                    "id": f"operation-{operator}",
                    "passed": actual >= minimum,
                }
            )
    check("encrypted", report["document"]["encrypted"], False)
    check(
        "action-kinds",
        [action["kind"] for action in report["document"]["actions"]],
        expectation["action_kinds"],
    )
    check(
        "prohibited-actions",
        [
            action["location"]
            for action in report["document"]["actions"]
            if action["classification"] == "prohibited"
        ],
        [],
    )
    check("signature-count", len(report["document"]["signatures"]), 0)
    check("incremental-revisions", report["document"]["incremental_revisions"], 0)
    check("linearized", report["document"]["linearized"], False)
    check("info-dictionary", report["document"]["info_dictionary"], None)
    check(
        "attachment-count",
        len(report["document"]["attachments"]),
        expectation["attachment_count"],
    )
    check(
        "xmp-present",
        report["document"]["xmp"] is not None,
        expectation["xmp_present"],
    )
    intents = report["document"]["output_intents"]
    check("output-intent-count", len(intents), expectation["output_intent_count"])
    if intents:
        profile = intents[0]["profile"] or {}
        check("output-profile-version", profile.get("version"), "2.1")
        check("output-profile-class", profile.get("device_class"), "mntr")
        check("output-profile-space", profile.get("color_space"), "RGB")
        check("output-profile-pcs", profile.get("pcs"), "XYZ")
        check("output-profile-signature", profile.get("signature"), "acsp")
        check("output-profile-header", profile.get("valid_header"), True)
        missing_tags = sorted(set(expectation["required_icc_tags"]) - set(profile.get("tags", [])))
        check("output-profile-tags", missing_tags, [])
    return checks


def inherited_page_value(objects: dict, page: dict, key: str, default):
    current = page
    seen = set()
    for _ in range(64):
        if key in current:
            return current[key]
        parent_ref = current.get("/Parent")
        if not isinstance(parent_ref, str) or parent_ref in seen:
            return default
        seen.add(parent_ref)
        current = resolve(objects, parent_ref)
    return default


def resolve(objects: dict, value) -> dict:
    resolved = resolve_value(objects, value)
    return resolved if isinstance(resolved, dict) else {}


def resolve_value(objects: dict, value):
    if isinstance(value, str) and value.endswith(" R"):
        record = objects.get(f"obj:{value}", {})
        return object_value(record)
    return value


def object_value(record):
    if not isinstance(record, dict):
        return record
    if "value" in record:
        return record["value"]
    if "stream" in record:
        return record["stream"].get("dict", {})
    return record


def stream_dict(objects: dict, value) -> dict:
    if not isinstance(value, str):
        return {}
    record = objects.get(f"obj:{value}", {})
    return record.get("stream", {}).get("dict", {})


def stream_value(objects: dict, value) -> bytes | None:
    if not isinstance(value, str):
        return None
    record = objects.get(f"obj:{value}", {})
    data = record.get("stream", {}).get("data")
    if not isinstance(data, str):
        return None
    return base64.b64decode(data, validate=True)


def stream_descriptor(data: bytes | None) -> dict | None:
    if data is None:
        return None
    return {"byte_length": len(data), "sha256": sha256(data)}


def reference(value):
    return value if isinstance(value, str) and value.endswith(" R") else None


def normalize(value):
    if isinstance(value, str) and value.startswith("u:"):
        return value[2:]
    if isinstance(value, list):
        return [normalize(item) for item in value]
    if isinstance(value, dict):
        return {key: normalize(item) for key, item in sorted(value.items())}
    return value


def sorted_dict(value):
    return sorted(value.items()) if isinstance(value, dict) else []


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


if __name__ == "__main__":
    raise SystemExit(main())
