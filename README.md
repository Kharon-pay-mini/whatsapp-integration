# Kharon Pay WhatsApp Bot 💰

> **Send crypto to your bank in seconds via WhatsApp**

Kharon Pay is a WhatsApp-based crypto offramp service that allows users to convert their cryptocurrency (USDT/USDC) on Starknet to Nigerian Naira (NGN) and receive funds directly in their bank accounts.

---

## 🚀 Getting Started

### Join the Bot

To start using Kharon Pay, you need to join the WhatsApp sandbox:

1. **Add the Twilio Sandbox number to your contacts:**
   ```
   +1 415 523 8886
   ```

2. **Send the following message to join:**
   ```
   join kept-equator
   ```

3. **Wait for confirmation** that you've successfully joined the sandbox.

4. **Start interacting** with the bot by sending `hi` or `start`

---

## ⚠️ Important Notes

- **Response Time**: The bot is deployed on Render's free tier, which may cause occasional delays. If you don't receive a response within **60 seconds**, please resend your message.
- **First Request Delay**: The first message may take longer as the server "wakes up" from sleep mode on free tier hosting.
- **Network**: Only **Starknet** network is supported for USDT/USDC deposits.
- **NOTE**: The bot is still in production mode, users may experience some inconsistencies.

---

## 📱 User Flow Guide

### 1️⃣ **Create Your Account**

**User:** `hi`

**Bot Response:**
```
🟢 Welcome to Kharon Pay! 💰

Send crypto to your bank in seconds.

📱 Commands:
• create - Create new account
• fund - Deposit crypto to your wallet address
• withdraw - Send crypto to your bank account
• balance - Check crypto balance in your wallet
• help - Show all commands

What would you like to do?
```

**User:** `create`

**Bot Response:**
```
🔄 Creating Your Account!

Please wait while we set up your wallet...
```

**Bot Response (after processing):**
```
0x03132292784cc21d0c4be34b55ab828303887922e649c59ec075261ead5b9609
```

**Bot Response:**
```
🎉 Account created successfully!

📱 To withdraw crypto:
• copy address - Copy your wallet address above
• fund account - Send crypto to your wallet address
• withdraw - Send crypto to your bank account
```
> **NOTE:** Address deployed and fully verifiable on https://sepolia.starkscan.co
---

### 2️⃣ **Get Your Wallet Address**

**User:** `address`

**Bot Response:**
```
0x03132292784cc21d0c4be34b55ab828303887922e649c59ec075261ead5b9609
```

**Bot Response:**
```
💳 Your Wallet Address:

⚠️ Only send USDT/USDC (Starknet) to this address
```

> **Note:** Copy this address and use it to send USDT or USDC on the Sepolia Starknet network.

---

### 3️⃣ **Fund Your Wallet**

1. Copy your wallet address from the bot
2. Send USDT or USDC on **Sepolia Starknet network** to this address
3. Wait for blockchain confirmation
4. Check your balance using the `balance` command

---

### 4️⃣ **Withdraw to Your Bank**

**User:** `withdraw 1 usdt`

**Bot Response:**
```
💸 Withdraw Request

Amount: 1.00 USDT
Rate: ₦1477.00 per USDT
You'll receive: ₦1477.00

Type confirm to proceed or cancel to abort.
```

**User:** `confirm`

**Bot Response:**
```
🏦 Bank Details Required

Please provide your bank details in this format:

Bank Name, Account Number

Example: Opay, 0123456789
```

**User:** `Opay, 0123456789`

**Bot Response:**
```
✅ Account Verified!

🏦 Bank: Opay
👤 Account Name: JOE DOE
🔢 Account Number: 0123456789

Is this correct?
Type yes to confirm or no to re-enter.
```

**User:** `yes`

**NOTE**: This process may not proceed further than this point as user needs to have a balance of whitelisted token(s) to continue. If user wishes to continue, he can request for token, accompanied by wallet address, via email at: mr.evans0075@gmail.com

**Bot Response:**
```
✅ Withdrawal Request Submitted!

📊 Details:
• Amount: 1.00 USDT
• Bank: Opay
• Account: 0123456789 (JOE DOE)

⏳ Processing time: 10-30 minutes
📱 You'll receive SMS confirmation when completed
```

---

## 🎯 Available Commands

| Command | Description | Example |
|---------|-------------|---------|
| `hi` / `hello` / `start` | Start the bot and see welcome message | `hi` |
| `create` | Create a new Kharon Pay account | `create` |
| `address` | Get your wallet address for deposits | `address` |
| `balance` | Check your crypto balance | `balance` |
| `withdraw [amount] [crypto]` | Initiate withdrawal to bank | `withdraw 100 usdt` |
| `help` | Show all available commands | `help` |

---

## 💡 Tips & Best Practices

### ✅ Do's
- **Always verify** your bank account details before confirming withdrawal
- **Use the correct network** (Starknet) when sending crypto to your wallet
- **Wait for confirmations** - blockchain transactions take time
- **Start with small amounts** when testing the service
- **Keep your wallet address safe** - it's your unique identifier

### ❌ Don'ts
- **Don't send crypto on wrong networks** (Ethereum, BSC, etc.) - funds will be lost
- **Don't panic** if responses are slow - the server may be waking up from sleep mode
- **Don't send minimum amounts** below 1 USDT/USDC

---

## 🔒 Security & Privacy
- Bank details are **securely stored** and only used for withdrawals
- All transactions are **verifiable** on the Starknet blockchain: https://sepolia.starkscan.co

---

## 🐛 Troubleshooting

### Bot not responding?
1. Wait 60 seconds (server may be starting up)
2. Resend your message
3. Check if you've joined the sandbox correctly

### Withdrawal not processed?
1. Check your balance first
2. Verify bank details are correct
3. Wait the full processing time (30-60 seconds)
4. Contact support if issue persists

### Wrong network used?
- Unfortunately, funds sent on wrong networks cannot be recovered
- Always double-check you're sending to **Starknet**
  
---

## 🌟 Features Coming Soon

- [ ] Multiple bank account support
- [ ] Transaction history via WhatsApp
- [ ] Support for more cryptocurrencies
- [ ] Faster processing times
- [ ] Rate alerts and notifications

---

## 📄 Terms of Service

By using Kharon Pay, you agree to:
- Comply with local cryptocurrency regulations
- Use the service for legitimate purposes only
- Accept the exchange rates provided at time of transaction
- Understand that cryptocurrency transactions are irreversible

---

## 🎉 Ready to Start?

1. **Join the sandbox:** Send `join kept-equator` to `+1 (415) 523-8886`
2. **Say hi:** Send `hi` to the bot
3. **Create account:** Type `create`
4. **Start trading!** Fund your wallet and withdraw to your bank

**Welcome to the future of crypto banking! 🚀**

---

*Made with ❤️ by the Kharon Pay Team*
