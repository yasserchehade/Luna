import type { Metadata } from "next";
import "./styles.css";

export const metadata: Metadata = {
  title: "Luna — Household administration",
  description: "Luna's briefing-led household administration experience.",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
