from django import forms
from django.conf import settings


class LineForm(forms.Form):
    sku = forms.CharField(max_length=32)
    quantity = forms.IntegerField(min_value=1)
    unit_cents = forms.IntegerField(min_value=0)


class OrderForm(forms.Form):
    note = forms.CharField(max_length=280, required=False)
    lines = forms.JSONField()

    def clean_lines(self) -> list[dict]:
        lines = self.cleaned_data["lines"]
        if not isinstance(lines, list) or not lines:
            raise forms.ValidationError("an order needs at least one line")
        if len(lines) > settings.ORDER_MAX_LINES:
            raise forms.ValidationError(f"an order has at most {settings.ORDER_MAX_LINES} lines")
        cleaned = []
        for line in lines:
            form = LineForm(line)
            if not form.is_valid():
                raise forms.ValidationError(f"line {line!r} is invalid")
            cleaned.append(form.cleaned_data)
        return cleaned
