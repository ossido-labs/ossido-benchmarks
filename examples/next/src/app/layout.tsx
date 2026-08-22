import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Next.js benchmark",
  description: "Ossido vs Next.js benchmark",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className="h-full antialiased">
      <body className="min-h-full flex flex-col bg-black">{children}</body>
    </html>
  );
}
