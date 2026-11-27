// Self-check for money display. Run: node src/lib/money.check.ts
import { cleanMoney, formatMoney, isMoneyInput, isOwing } from './money.ts';

const cases: [unknown, unknown][] = [
  [formatMoney('1500000.00', 'UGX'), 'UGX 1,500,000'],
  [formatMoney('12.5', 'USD'), 'USD 12.50'],
  [formatMoney('0', 'UGX'), 'UGX 0'],
  [formatMoney('007.125', 'KES'), 'KES 7.125'],
  [formatMoney('999', 'UGX'), 'UGX 999'],
  [formatMoney('1234567.89', 'UGX'), 'UGX 1,234,567.89'],
  [formatMoney('n/a', 'UGX'), 'UGX n/a'],
  [isOwing('0.00'), false],
  [isOwing('0'), false],
  [isOwing(''), false],
  [isOwing('-5'), false],
  [isOwing('0.01'), true],
  [isOwing('150000'), true],
  [isMoneyInput(' 1500 '), true],
  [isMoneyInput('1,500'), true],
  [isMoneyInput('150,000'), true],
  [isMoneyInput('1,50'), false],
  [isMoneyInput('15,00,000'), false],
  [cleanMoney(' 1,500,000 '), '1500000'],
  [cleanMoney('1,250,000.50'), '1250000.50'],
  [cleanMoney('1,50'), '1,50'],
  [cleanMoney('450000'), '450000'],
  [isMoneyInput('0'), false],
  [isMoneyInput('-3'), false],
];
cases.forEach(([got, want], i) => {
  if (got !== want) throw new Error(`case ${i}: got ${String(got)}, want ${String(want)}`);
});
console.log(`money: ${cases.length} checks passed`);
