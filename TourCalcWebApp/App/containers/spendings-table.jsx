import React from 'react';
import AppState from './appstate.jsx'

import { BrowserRouter as Router, Route, Switch, Link, Redirect } from 'react-router-dom';

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';
import Paper from '@material-ui/core/Paper';

import PersonForm from './tour-person-edit.jsx'
import SpendingForm from './tour-spending-edit.jsx'
import SpendingsDetail from './person-show-spendings.jsx'

import Checkbox from '@material-ui/core/Checkbox';

import createBrowserHistory from 'history/createBrowserHistory';

import Tabs from '@material-ui/core/Tabs';
import Tab from '@material-ui/core/Tab';
import Button from '@material-ui/core/Button';
import Box from '@material-ui/core/Box';


export default class SpendingTable extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            tour: props.tour
        }
    }
    componentDidMount() {
        alert("did mount");
    }
    render() {
        return (

            <Table stickyHeader>
                <TableHead>
                    <TableRow>
                        <TableCell>Spending Description
                       
                                                            <SpendingForm
                                tour={this.state.tour}
                                buttonText="Add"
                                actionButtonText="Add Spending"
                                open={false}
                                mode="add"
                                app={this.props.app}
                            ><Button color='primary' variant='outlined'>Add</Button></SpendingForm>

                        </TableCell>
                        <TableCell align="right">From</TableCell>
                        <TableCell align="right">Amount</TableCell>
                        <TableCell align="right">To all</TableCell>
                        <TableCell align="right">Recipients</TableCell>
                    </TableRow>
                </TableHead>
                <TableBody>
                    {this.state.tour.spendings.map((p, idx) => (
                        <TableRow key={p.guid} hover>
                            <TableCell component="th" scope="row">
                                <span style={{ cursor: 'pointer', borderStyle: 'ridge', fontSize: 'xx-small' }} variant='outlined' color='secondary' onClick={() => {
                                    if (window.confirm('Sure to delete spending ' + p.description + '?')) {
                                        AppState.deleteSpending(this.props.app, this.state.tour.id, p.guid)
                                            .then(() => { AppState.loadTour(this.props.app, this.state.tour.id); })
                                    }
                                }}>X</span>
                                &nbsp;
                                                                {(idx + 1) + '.'}
                                <SpendingForm
                                    tour={this.state.tour}
                                    buttonText={p.description}
                                    actionButtonText="Save Spending"
                                    open={false}
                                    mode="edit"
                                    app={this.props.app}
                                    spending={p}
                                ><span style={{ cursor: 'pointer', textDecoration: 'underline' }}>{p.description}</span></SpendingForm>
                            </TableCell>
                            <TableCell align="right">{
                                this.state.tour.persons.filter((pp) => pp.guid === p.fromGuid).map(ppp => ppp.name)
                            }</TableCell>
                            <TableCell align="right">
                                {p.amountInCents}
                            </TableCell>
                            <TableCell align="right"><Checkbox checked={p.toAll} disabled /></TableCell>
                            <TableCell align="right" style={{ fontSize: "xx-small" }}>{p.toAll ? (<b>n/a</b>) : p.toGuid.map(

                                (id) => (this.state.tour.persons.find((pp) => pp.guid === id) == null)
                                    ? '$$-' + id + '-$$'
                                    : this.state.tour.persons.find((pp) => pp.guid === id).name

                            ).join(', ')}</TableCell>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>


            );
    }

}