import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { ListPaymentsFilter, PaymentInput, PaymentRow, ReceiptInput } from './types.ts';

export async function recordPaymentWithReceipt(
  input: PaymentInput,
): Promise<[number, string]> {
  try {
    return await invoke<[number, string]>('record_payment_and_generate_receipt', {
      request: input,
    });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function generateAndOpenReceipt(input: ReceiptInput): Promise<string> {
  try {
    return await invoke<string>('generate_and_open_receipt', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listPayments(input: ListPaymentsFilter): Promise<PaymentRow[]> {
  try {
    return await invoke<PaymentRow[]>('list_payments', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}
