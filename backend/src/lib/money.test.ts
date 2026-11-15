import { describe, expect, test } from "bun:test";
import { parseMoney, parsePositiveMoney } from "./money";

describe("parseMoney", () => {
  test("parses valid amounts", () => {
    expect(parseMoney("150.50")).toEqual({ ok: true, value: "150.50" });
    expect(parseMoney("  49.50  ")).toEqual({ ok: true, value: "49.50" });
    expect(parseMoney("0.00")).toEqual({ ok: true, value: "0.00" });
    expect(parseMoney("1200000")).toEqual({ ok: true, value: "1200000" });
  });

  test("rejects negative amounts", () => {
    expect(parseMoney("-10.00")).toEqual({
      ok: false,
      error: { code: "negative", message: "amount must not be negative" },
    } as const);
  });

  test("rejects invalid decimals", () => {
    const invalid = {
      code: "invalid_decimal",
      message: "amount must be a valid decimal amount",
    } as const;
    expect(parseMoney("")).toEqual({ ok: false, error: invalid });
    expect(parseMoney("abc")).toEqual({ ok: false, error: invalid });
    expect(parseMoney("12.34.56")).toEqual({ ok: false, error: invalid });
    expect(parseMoney("1,000")).toEqual({ ok: false, error: invalid });
    expect(parseMoney("12.5 USD")).toEqual({ ok: false, error: invalid });
    expect(parseMoney("NaN")).toEqual({ ok: false, error: invalid });
    expect(parseMoney("Infinity")).toEqual({ ok: false, error: invalid });
    expect(parseMoney("0x10")).toEqual({ ok: false, error: invalid });
  });

  test("never returns a float", () => {
    const result = parseMoney("0.1");
    expect(result.ok).toBe(true);
    if (result.ok) expect(typeof result.value).toBe("string");
  });
});

describe("parsePositiveMoney", () => {
  test("rejects zero", () => {
    const zero = { code: "zero", message: "amount must be greater than zero" } as const;
    expect(parsePositiveMoney("0.00")).toEqual({ ok: false, error: zero });
    expect(parsePositiveMoney("0")).toEqual({ ok: false, error: zero });
    expect(parsePositiveMoney("00.000")).toEqual({ ok: false, error: zero });
  });

  test("accepts positive amounts", () => {
    expect(parsePositiveMoney("0.01")).toEqual({ ok: true, value: "0.01" });
    expect(parsePositiveMoney("150.50")).toEqual({ ok: true, value: "150.50" });
  });

  test("still rejects negatives and garbage", () => {
    expect(parsePositiveMoney("-5").ok).toBe(false);
    expect(parsePositiveMoney("nope").ok).toBe(false);
  });
});
