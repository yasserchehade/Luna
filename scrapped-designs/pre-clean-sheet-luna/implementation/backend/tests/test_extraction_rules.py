from app.services.extraction import (
    _build_supplier_profile_match,
    _classify_category,
    _extract_amount,
    _extract_due_date,
    _extract_invoice_number,
    _extract_supplier,
)


def test_agl_side_by_side_bill_summary() -> None:
    text = """
    Amount due
    Due date
    AGL Sales Pty Ltd ABN 88 090 538 337
    Issue date
    5 Apr 2023
    Account number
    123 4567 891X
    Tax Invoice
    $63.70
    23 Apr 2023
    """

    supplier = _extract_supplier(text, "agl.pdf")

    assert supplier == "AGL Sales Pty Ltd"
    assert _extract_amount(text) == 63.70
    assert _extract_due_date(text) == "2023-04-23"
    assert _classify_category(text, supplier) == "utilities"
    assert _build_supplier_profile_match(text, supplier)["template_status"] == "known"


def test_tango_energy_clean_invoice_labels() -> None:
    text = """
    Pacific Blue Retail Pty Ltd t/a Tango Energy
    TAX INVOICE ELECTRICITY
    Account number
    1234567
    Tax invoice number 7654321
    Due date 16 Oct 2023
    Total amount due $169.46
    NATIONAL METER IDENTIFIER (NMI) 12345678901
    """

    supplier = _extract_supplier(text, "tango.pdf")

    assert supplier == "Tango Energy"
    assert _extract_invoice_number(text) == "7654321"
    assert _extract_amount(text) == 169.46
    assert _extract_due_date(text) == "2023-10-16"
    assert _build_supplier_profile_match(text, supplier)["template_status"] == "known"


def test_water_corporation_please_pay_due_by_labels() -> None:
    text = """
    ACCOUNT NUMBER 90 99999 99 9
    WATER USE PERIOD 63 DAYS
    BILL ID 0184
    ISSUE DATE 11 OCT 2021
    PLEASE PAY:
    $223.71
    DUE BY:
    27 Oct 2021
    watercorporation.com.au/billhelp
    """

    supplier = _extract_supplier(text, "water.pdf")

    assert supplier == "Water Corporation"
    assert _extract_invoice_number(text) == "0184"
    assert _extract_amount(text) == 223.71
    assert _extract_due_date(text) == "2021-10-27"
    assert _build_supplier_profile_match(text, supplier)["template_status"] == "known"


def test_yarra_valley_water_total_balance_layout() -> None:
    text = """
    Total this bill (GST does not apply) $266.64
    Total balance $266.64
    Account number
    Invoice number
    Issue date 8 May 2026
    Tax Invoice Yarra Valley Water ABN 93 066 902 501
    Amount due
    $266.64
    Due date
    29 MAY 2026
    """

    supplier = _extract_supplier(text, "utility bill.pdf")

    assert supplier == "Yarra Valley Water"
    assert _extract_amount(text) == 266.64
    assert _extract_due_date(text) == "2026-05-29"
    assert _build_supplier_profile_match(text, supplier)["template_status"] == "known"


def test_insurance_relative_payment_terms() -> None:
    text = """
    IMC Insurance Brokers
    NEW BUSINESS TAX INVOICE
    payment is required within 14 days from 11/06/2025.
    Policy Number 132A123717HOP
    Invoice No: 291727
    Total Due: $2,405.08
    """

    supplier = _extract_supplier(text, "insurance_house.pdf")

    assert supplier == "IMC Insurance Brokers"
    assert _extract_invoice_number(text) == "291727"
    assert _extract_amount(text) == 2405.08
    assert _extract_due_date(text) == "2025-06-25"
    assert _classify_category(text, supplier) == "insurance"
