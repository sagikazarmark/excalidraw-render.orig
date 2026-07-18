function assertValidString(value) {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) {
        throw new TypeError("canonical JSON cannot contain an unpaired surrogate");
      }
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      throw new TypeError("canonical JSON cannot contain an unpaired surrogate");
    }
  }
}

export function canonicalize(value) {
  if (value === null || typeof value === "boolean") {
    return JSON.stringify(value);
  }
  if (typeof value === "string") {
    assertValidString(value);
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError("canonical JSON numbers must be finite");
    }
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalize).join(",")}]`;
  }
  if (typeof value === "object") {
    const members = Object.keys(value)
      .sort()
      .map((key) => {
        assertValidString(key);
        const member = value[key];
        if (member === undefined || typeof member === "function" || typeof member === "symbol") {
          throw new TypeError(`canonical JSON does not support ${typeof member} values`);
        }
        return `${JSON.stringify(key)}:${canonicalize(member)}`;
      });
    return `{${members.join(",")}}`;
  }
  throw new TypeError(`canonical JSON does not support ${typeof value} values`);
}
