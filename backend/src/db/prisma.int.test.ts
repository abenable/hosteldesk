/**
 * Integration tests against the local Postgres database (DATABASE_URL),
 * using the bun-flavor Prisma client. Skipped when DATABASE_URL is unset.
 */
import { describe, expect, test } from "bun:test";
import { createPrisma } from "./prisma-bun";
import type { DbClient } from "./prisma-cf";

const hasDb = Boolean(process.env.DATABASE_URL);

/** Await `fn` and report whether it threw. (Bun's .rejects matcher doesn't
 * accept PrismaPromise, which is a thenable but not a native Promise.) */
async function rejects(fn: () => Promise<unknown>): Promise<boolean> {
  try {
    await fn();
    return false;
  } catch {
    return true;
  }
}

function makeDb(): DbClient {
  return createPrisma({ DATABASE_URL: process.env.DATABASE_URL! });
}

describe("prisma + local postgres", () => {
  test.skipIf(!hasDb)("money round-trips as exact decimal strings", async () => {
    const db = makeDb();
    let hostelId = 0;
    try {
      const hostel = await db.hostel.create({ data: { name: "__test_hostel__" } });
      hostelId = hostel.id;

      const room = await db.room.create({
        data: {
          hostelId: hostel.id,
          roomNumber: "T1",
          semesterRate: "500000.00",
          recessRate: "250000.50",
        },
      });
      // Money comes back as exact decimal.js values — never a float. Postgres
      // NUMERIC + decimal.js normalize trailing zeros ("500000.00" → "500000"),
      // so assert on the exact scaled value rather than the literal's spelling.
      expect(room.semesterRate.toFixed(2)).toBe("500000.00");
      expect(room.recessRate.toFixed(2)).toBe("250000.50");

      // Aggregation happens in SQL and is exact: 0.1 + 0.2 === 0.3.
      await db.room.create({
        data: { hostelId: hostel.id, roomNumber: "T2", semesterRate: "0.1", recessRate: "0.1" },
      });
      await db.room.create({
        data: { hostelId: hostel.id, roomNumber: "T3", semesterRate: "0.2", recessRate: "0.2" },
      });

      const sum = await db.room.aggregate({
        where: { hostelId: hostel.id },
        _sum: { semesterRate: true },
      });
      expect(sum._sum.semesterRate?.toString()).toBe("500000.3");
    } finally {
      await db.hostel.delete({ where: { id: hostelId } });
      await db.$disconnect();
    }
  });

  test.skipIf(!hasDb)("DB CHECK constraints reject non-positive money", async () => {
    const db = makeDb();
    let hostelId = 0;
    try {
      const hostel = await db.hostel.create({ data: { name: "__test_hostel2__" } });
      hostelId = hostel.id;
      const threw = await rejects(() =>
        db.room.create({
          data: { hostelId: hostel.id, roomNumber: "T1", semesterRate: "0", recessRate: "1" },
        }),
      );
      expect(threw).toBe(true);
    } finally {
      await db.hostel.delete({ where: { id: hostelId } });
      await db.$disconnect();
    }
  });

  test.skipIf(!hasDb)("currency CHECK enforces uppercase ISO-style codes", async () => {
    const db = makeDb();
    const threw = await rejects(() =>
      db.hostel.create({ data: { name: "__test_bad_currency__", currency: "ugx" } }),
    );
    expect(threw).toBe(true);
    await db.$disconnect();
  });
});
