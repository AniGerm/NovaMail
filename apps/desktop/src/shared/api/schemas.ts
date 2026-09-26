import { z } from "zod";

export const addressSchema = z.object({
  name: z.string().nullish(),
  email: z.string().email(),
});

export const addAccountPasswordSchema = z.object({
  name: z.string().min(1),
  email: z.string().email(),
  password: z.string().min(1),
  provider: z.enum([
    "generic",
    "gmail",
    "microsoft365",
    "yahoo",
    "protonBridge",
  ]),
  imapHost: z.string().min(1),
  imapPort: z.number().int().min(1).max(65535),
  imapTls: z.boolean(),
  smtpHost: z.string().min(1),
  smtpPort: z.number().int().min(1).max(65535),
  smtpTls: z.boolean(),
});

export const sendMessageSchema = z.object({
  accountId: z.string().uuid(),
  to: z.array(addressSchema).min(1),
  cc: z.array(addressSchema),
  bcc: z.array(addressSchema),
  subject: z.string(),
  bodyText: z.string().min(1),
  bodyHtml: z.string().nullish(),
  inReplyTo: z.string().nullish(),
  references: z.array(z.string()),
});

export type AddAccountPasswordInput = z.infer<typeof addAccountPasswordSchema>;
export type SendMessageInput = z.infer<typeof sendMessageSchema>;
