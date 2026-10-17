#pragma once
#include <QSortFilterProxyModel>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

// Presentation-only sorting/filtering; never used to decide authorization.
class RowFilter : public QSortFilterProxyModel {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString query READ query WRITE setQuery NOTIFY queryChanged)
    Q_PROPERTY(QString sortField READ sortField WRITE setSortField NOTIFY sortFieldChanged)
    Q_PROPERTY(bool descending READ descending WRITE setDescending NOTIFY descendingChanged)
    Q_PROPERTY(QString roleFilter READ roleFilter WRITE setRoleFilter NOTIFY roleFilterChanged)
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
public:
    explicit RowFilter(QObject *parent = nullptr) : QSortFilterProxyModel(parent) {
        setDynamicSortFilter(true);
        sort(0);
        connect(this, &QAbstractItemModel::rowsInserted, this, &RowFilter::countChanged);
        connect(this, &QAbstractItemModel::rowsRemoved, this, &RowFilter::countChanged);
        connect(this, &QAbstractItemModel::modelReset, this, &RowFilter::countChanged);
    }
    QString query() const { return m_query; }
    QString sortField() const { return m_sortField; }
    QString roleFilter() const { return m_roleFilter; }
    bool descending() const { return sortOrder() == Qt::DescendingOrder; }
    void setQuery(QString value) {
        if (m_query == value) return;
        beginFilterChange(); m_query = value; endFilterChange(Direction::Rows); emit queryChanged();
    }
    void setRoleFilter(QString value) {
        if (m_roleFilter == value) return;
        beginFilterChange(); m_roleFilter = value; endFilterChange(Direction::Rows); emit roleFilterChanged();
    }
    void setSortField(QString value) {
        if (m_sortField == value) return;
        m_sortField = value; invalidate(); emit sortFieldChanged();
    }
    void setDescending(bool value) {
        if (descending() == value) return;
        sort(0, value ? Qt::DescendingOrder : Qt::AscendingOrder); emit descendingChanged();
    }
    Q_INVOKABLE QVariantMap get(int row) const { return data(index(row, 0), Qt::UserRole).toMap(); }
protected:
    bool filterAcceptsRow(int row, const QModelIndex &parent) const override {
        const auto value = sourceModel()->data(sourceModel()->index(row, 0, parent), Qt::UserRole).toMap();
        if (!m_roleFilter.isEmpty() && !value.value("roles").toStringList().contains(m_roleFilter)) return false;
        const auto query = m_query.trimmed();
        return query.isEmpty() || value.value("name").toString().contains(query, Qt::CaseInsensitive)
            || value.value("id").toString().contains(query, Qt::CaseInsensitive)
            || value.value("roles").toStringList().join(" ").contains(query, Qt::CaseInsensitive);
    }
    bool lessThan(const QModelIndex &left, const QModelIndex &right) const override {
        const auto a = sourceModel()->data(left, Qt::UserRole).toMap();
        const auto b = sourceModel()->data(right, Qt::UserRole).toMap();
        if (m_sortField == "priority" && a.value(m_sortField).toUInt() != b.value(m_sortField).toUInt())
            return a.value(m_sortField).toUInt() < b.value(m_sortField).toUInt();
        const auto comparison = QString::compare(a.value(m_sortField).toString(), b.value(m_sortField).toString(), Qt::CaseInsensitive);
        return comparison == 0 ? a.value("id").toString() < b.value("id").toString() : comparison < 0;
    }
signals:
    void queryChanged();
    void sortFieldChanged();
    void descendingChanged();
    void roleFilterChanged();
    void countChanged();
private:
    QString m_query, m_roleFilter;
    QString m_sortField = "name";
};
