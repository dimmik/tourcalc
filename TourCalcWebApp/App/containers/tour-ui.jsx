import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import { BrowserRouter as Router, Route, Switch, Link } from 'react-router-dom';

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';
import Paper from '@material-ui/core/Paper';

import PersonForm from './tour-edit.jsx'



export default class TourUI extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false,
        }
    }

    componentDidMount() {
        AppState.checkWhoAmI(this);
    }

    render() {
        if (!this.state.isAuthLoaded) {
            return (<div>Checking Who you are...</div>)
        } else {
            if (!this.state.authData.isMaster
                && this.state.authData.tourIds.indexOf(this.props.tourid) == -1) { // no such tour for credentials
                return (<div><pre>{JSON.stringify(this.state.authData, null, 2)}</pre><TourRequestAccessCode app={this} /></div>)
            } else {
                return (<TourTable tourid={this.props.tourid} />)
            }

        }
        
    }
}
/*const useStyles = makeStyles(theme => ({
    root: {
        width: '100%',
        marginTop: theme.spacing(3),
        overflowX: 'auto',
    },
    table: {
        minWidth: 650,
    },
}));
const classes = useStyles();*/

class TourTable extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isTourLoaded: false,
            tour: null,
            dialogPOpen: false
        }
    }

    componentDidMount() {
        AppState.loadTour(this, this.props.tourid);
    }

    render() {
        if (!this.state.isTourLoaded) {
            return <div>Tour {this.props.tourid} loading...</div>
        } else {
            return (
                <Router>
                    <div>
                        <h2>{this.state.tour.name}</h2>
                        <Link to={'/tour/' + this.props.tourid + '/persons'}>Persons</Link>&nbsp;
                        <Link to={'/tour/' + this.props.tourid + '/spendings'}>Spendings</Link>
                        <span><button onClick={() => { this.setState({ isTourLoaded: false }); AppState.loadTour(this, this.props.tourid); }}>Refresh</button> </span>

                        <main>
                            <Switch>
                                <Route path={'/tour/' + this.props.tourid + '/spendings'}
                                    render={(props) => (

                                        <Paper>
                                            <Table stickyHeader>
                                                <TableHead>
                                                    <TableRow>
                                                        <TableCell>Reason
                                                            
                                                        </TableCell>
                                                        <TableCell align="right">From</TableCell>
                                                        <TableCell align="right">Amount</TableCell>
                                                        <TableCell align="right">To all?</TableCell>
                                                        <TableCell align="right">Recipients</TableCell>
                                                    </TableRow>
                                                </TableHead>
                                                <TableBody>
                                                    {this.state.tour.spendings.map(p => (
                                                        <TableRow key={p.guid} hover>
                                                            <TableCell component="th" scope="row">
                                                                {p.description}
                                                            </TableCell>
                                                            <TableCell align="right">{this.state.tour.persons.find((pp) => pp.guid === p.fromGuid).name}</TableCell>
                                                            <TableCell align="right">{p.amountInCents / 100}</TableCell>
                                                            <TableCell align="right">{p.toAll ? 'true' : 'false'}</TableCell>
                                                            <TableCell align="right">{p.toGuid.map(

                                                                (id) => this.state.tour.persons.find((pp) => pp.guid === id).name

                                                            ).join(', ')}</TableCell>
                                                        </TableRow>  
                                                    ))}
                                                </TableBody>
                                            </Table>
                                        </Paper>


                                    )} />
                                <Route path={'/tour/' + this.props.tourid}
                                    render={(props) => (
                                        <Paper>
                                            <Table stickyHeader>
                                                <TableHead>
                                                    <TableRow>
                                                        <TableCell>Name (<PersonForm tourid={this.props.tourid} app={this} open={this.state.dialogPOpen} buttonText="Add" actionButtonText="Add Person"/>)</TableCell>
                                                        <TableCell align="right">Weight</TableCell>
                                                        <TableCell align="right">Spent</TableCell>
                                                        <TableCell align="right">Received</TableCell>
                                                        <TableCell align="right">Owes</TableCell>
                                                    </TableRow>
                                                </TableHead>
                                                <TableBody>
                                                    {this.state.tour.persons.map(p => (
                                                        <TableRow key={p.guid} hover>
                                                            <TableCell component="th" scope="row">
                                                                {p.name}
                                                            </TableCell>
                                                            <TableCell align="right">{p.weight / 100}</TableCell>
                                                            <TableCell align="right">{p.spentInCents / 100}</TableCell>
                                                            <TableCell align="right">{p.receivedInCents / 100}</TableCell>
                                                            <TableCell align="right">{(p.receivedInCents - p.spentInCents) / 100}</TableCell>
                                                        </TableRow>
                                                    ))}
                                                </TableBody>
                                            </Table>
                                        </Paper>
                                    )} />
                            </Switch>
                        </main>
                    </div>
                </Router>

                )
            //return <div><pre>Tour: {JSON.stringify(this.state.tour, null, 2)}}</pre></div>
        }
        function TPersons() {
            return <div> persons: <pre>{JSON.stringify(this.state.tour.persons, null, 2)}}</pre></div>
        }
        function TSpendings() {
            return <div> spendings: <pre>{JSON.stringify(this.state.tour.spendings, null, 2)}}</pre></div>
        }
    }

}

class TourRequestAccessCode extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false
        }
    }

    code = 'wrong_code'

    render() {
        return (
            <div>
                <form onSubmit={(event) => {
                    event.preventDefault();
                    AppState.login(this.props.app, 'code', this.code)
                }}>
                    <p>access code:</p>
                    <input
                        type='text'
                        onChange={(e) => this.code = event.target.value}
                    />
                    <input
                        type='submit'
                    />
                </form>
            </div>
            )
    }

}